// This file was generated from JSON Schema using quicktype, do not modify it directly.
// To parse the JSON, add this file to your project and do:
//
//   let types = try Types(json)

import Foundation

// MARK: - Types
struct Types: Codable {
    let actions: [Action]
    let events: Bool
    let queries: [Query]
    let types: TypesClass
}

// MARK: Types convenience initializers and mutators

extension Types {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(Types.self, from: data)
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
        actions: [Action]? = nil,
        events: Bool? = nil,
        queries: [Query]? = nil,
        types: TypesClass? = nil
    ) -> Types {
        return Types(
            actions: actions ?? self.actions,
            events: events ?? self.events,
            queries: queries ?? self.queries,
            types: types ?? self.types
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - Action
struct Action: Codable {
    let input: Bool
    let method: String
    let output: Bool
    let type: ActionType
}

// MARK: Action convenience initializers and mutators

extension Action {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(Action.self, from: data)
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
        input: Bool? = nil,
        method: String? = nil,
        output: Bool? = nil,
        type: ActionType? = nil
    ) -> Action {
        return Action(
            input: input ?? self.input,
            method: method ?? self.method,
            output: output ?? self.output,
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

enum ActionType: String, Codable {
    case coreAction = "core_action"
    case libraryAction = "library_action"
}

// MARK: - Query
struct Query: Codable {
    let input: JSONNull?
    let method: String
    let output: OutputUnion
    let type: QueryType
}

// MARK: Query convenience initializers and mutators

extension Query {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(Query.self, from: data)
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
        input: JSONNull?? = nil,
        method: String? = nil,
        output: OutputUnion? = nil,
        type: QueryType? = nil
    ) -> Query {
        return Query(
            input: input ?? self.input,
            method: method ?? self.method,
            output: output ?? self.output,
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

enum OutputUnion: Codable {
    case bool(Bool)
    case outputClass(OutputClass)

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let x = try? container.decode(Bool.self) {
            self = .bool(x)
            return
        }
        if let x = try? container.decode(OutputClass.self) {
            self = .outputClass(x)
            return
        }
        throw DecodingError.typeMismatch(OutputUnion.self, DecodingError.Context(codingPath: decoder.codingPath, debugDescription: "Wrong type for OutputUnion"))
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .bool(let x):
            try container.encode(x)
        case .outputClass(let x):
            try container.encode(x)
        }
    }
}

// MARK: - OutputClass
struct OutputClass: Codable {
    let properties: OutputProperties
    let outputRequired: [String]
    let title: String
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case outputRequired = "required"
        case title, type
    }
}

// MARK: OutputClass convenience initializers and mutators

extension OutputClass {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(OutputClass.self, from: data)
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
        properties: OutputProperties? = nil,
        outputRequired: [String]? = nil,
        title: String? = nil,
        type: OutputType? = nil
    ) -> OutputClass {
        return OutputClass(
            properties: properties ?? self.properties,
            outputRequired: outputRequired ?? self.outputRequired,
            title: title ?? self.title,
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

// MARK: - OutputProperties
struct OutputProperties: Codable {
    let builtAt: Destination
    let deviceInfo: ContentDuration
    let libraries: LibrariesClass
    let libraryCount: CapacityFree
    let network, services, system: ContentDuration
    let version: Destination

    enum CodingKeys: String, CodingKey {
        case builtAt = "built_at"
        case deviceInfo = "device_info"
        case libraries
        case libraryCount = "library_count"
        case network, services, system, version
    }
}

// MARK: OutputProperties convenience initializers and mutators

extension OutputProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(OutputProperties.self, from: data)
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
        builtAt: Destination? = nil,
        deviceInfo: ContentDuration? = nil,
        libraries: LibrariesClass? = nil,
        libraryCount: CapacityFree? = nil,
        network: ContentDuration? = nil,
        services: ContentDuration? = nil,
        system: ContentDuration? = nil,
        version: Destination? = nil
    ) -> OutputProperties {
        return OutputProperties(
            builtAt: builtAt ?? self.builtAt,
            deviceInfo: deviceInfo ?? self.deviceInfo,
            libraries: libraries ?? self.libraries,
            libraryCount: libraryCount ?? self.libraryCount,
            network: network ?? self.network,
            services: services ?? self.services,
            system: system ?? self.system,
            version: version ?? self.version
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - Destination
struct Destination: Codable {
    let type: DestinationType
}

// MARK: Destination convenience initializers and mutators

extension Destination {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(Destination.self, from: data)
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
        type: DestinationType? = nil
    ) -> Destination {
        return Destination(
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

enum DestinationType: String, Codable {
    case boolean = "boolean"
    case string = "string"
}

// MARK: - ContentDuration
struct ContentDuration: Codable {
    let ref: String

    enum CodingKeys: String, CodingKey {
        case ref = "$ref"
    }
}

// MARK: ContentDuration convenience initializers and mutators

extension ContentDuration {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(ContentDuration.self, from: data)
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
        ref: String? = nil
    ) -> ContentDuration {
        return ContentDuration(
            ref: ref ?? self.ref
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - LibrariesClass
struct LibrariesClass: Codable {
    let items: ContentDuration
    let type: String
}

// MARK: LibrariesClass convenience initializers and mutators

extension LibrariesClass {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(LibrariesClass.self, from: data)
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
        items: ContentDuration? = nil,
        type: String? = nil
    ) -> LibrariesClass {
        return LibrariesClass(
            items: items ?? self.items,
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

// MARK: - CapacityFree
struct CapacityFree: Codable {
    let format: Format
    let minimum: Double?
    let type: CapacityFreeType
    let description: String?
}

// MARK: CapacityFree convenience initializers and mutators

extension CapacityFree {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(CapacityFree.self, from: data)
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
        format: Format? = nil,
        minimum: Double?? = nil,
        type: CapacityFreeType? = nil,
        description: String?? = nil
    ) -> CapacityFree {
        return CapacityFree(
            format: format ?? self.format,
            minimum: minimum ?? self.minimum,
            type: type ?? self.type,
            description: description ?? self.description
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

enum Format: String, Codable {
    case dateTime = "date-time"
    case double = "double"
    case float = "float"
    case uint = "uint"
    case uint32 = "uint32"
    case uint64 = "uint64"
    case uuid = "uuid"
}

enum CapacityFreeType: String, Codable {
    case integer = "integer"
    case number = "number"
    case string = "string"
}

enum OutputType: String, Codable {
    case object = "object"
    case string = "string"
}

enum QueryType: String, Codable {
    case query = "query"
}

// MARK: - TypesClass
struct TypesClass: Codable {
    let apfsContainer: TypesApfsContainer
    let apfsVolumeInfo: TypesApfsVolumeInfo
    let apfsVolumeRole: TypesApfsVolumeRole
    let diskType: TypesDiskType
    let event: Event
    let fileCopyActionOutput: FileCopyActionOutput
    let fileOperation: FileOperation
    let fileSystem: TypesApfsVolumeRole
    let fsRawEventKind: TypesFSRawEventKind
    let genericProgress: TypesGenericProgress
    let indexerMetrics: TypesIndexerMetrics
    let indexerStats: TypesIndexerStats
    let jobInfoOutput: JobInfoOutput
    let jobOutput: TypesJobOutput
    let jobStatus: TypesDiskType
    let locationAddOutput: LocationAddOutput
    let mountType: TypesDiskType
    let pathMapping: TypesPathMapping
    let performanceMetrics: TypesPerformanceMetrics
    let progress: Progress
    let progressCompletion: TypesProgressCompletion
    let sdPath: TypesSDPath
    let sdPathBatch: SDPathBatch
    let volume: TypesVolume
    let volumeFingerprint: FileOperation
    let volumeInfo: TypesVolumeInfo
    let volumeType: TypesDiskType

    enum CodingKeys: String, CodingKey {
        case apfsContainer = "ApfsContainer"
        case apfsVolumeInfo = "ApfsVolumeInfo"
        case apfsVolumeRole = "ApfsVolumeRole"
        case diskType = "DiskType"
        case event = "Event"
        case fileCopyActionOutput = "FileCopyActionOutput"
        case fileOperation = "FileOperation"
        case fileSystem = "FileSystem"
        case fsRawEventKind = "FsRawEventKind"
        case genericProgress = "GenericProgress"
        case indexerMetrics = "IndexerMetrics"
        case indexerStats = "IndexerStats"
        case jobInfoOutput = "JobInfoOutput"
        case jobOutput = "JobOutput"
        case jobStatus = "JobStatus"
        case locationAddOutput = "LocationAddOutput"
        case mountType = "MountType"
        case pathMapping = "PathMapping"
        case performanceMetrics = "PerformanceMetrics"
        case progress = "Progress"
        case progressCompletion = "ProgressCompletion"
        case sdPath = "SdPath"
        case sdPathBatch = "SdPathBatch"
        case volume = "Volume"
        case volumeFingerprint = "VolumeFingerprint"
        case volumeInfo = "VolumeInfo"
        case volumeType = "VolumeType"
    }
}

// MARK: TypesClass convenience initializers and mutators

extension TypesClass {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(TypesClass.self, from: data)
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
        apfsContainer: TypesApfsContainer? = nil,
        apfsVolumeInfo: TypesApfsVolumeInfo? = nil,
        apfsVolumeRole: TypesApfsVolumeRole? = nil,
        diskType: TypesDiskType? = nil,
        event: Event? = nil,
        fileCopyActionOutput: FileCopyActionOutput? = nil,
        fileOperation: FileOperation? = nil,
        fileSystem: TypesApfsVolumeRole? = nil,
        fsRawEventKind: TypesFSRawEventKind? = nil,
        genericProgress: TypesGenericProgress? = nil,
        indexerMetrics: TypesIndexerMetrics? = nil,
        indexerStats: TypesIndexerStats? = nil,
        jobInfoOutput: JobInfoOutput? = nil,
        jobOutput: TypesJobOutput? = nil,
        jobStatus: TypesDiskType? = nil,
        locationAddOutput: LocationAddOutput? = nil,
        mountType: TypesDiskType? = nil,
        pathMapping: TypesPathMapping? = nil,
        performanceMetrics: TypesPerformanceMetrics? = nil,
        progress: Progress? = nil,
        progressCompletion: TypesProgressCompletion? = nil,
        sdPath: TypesSDPath? = nil,
        sdPathBatch: SDPathBatch? = nil,
        volume: TypesVolume? = nil,
        volumeFingerprint: FileOperation? = nil,
        volumeInfo: TypesVolumeInfo? = nil,
        volumeType: TypesDiskType? = nil
    ) -> TypesClass {
        return TypesClass(
            apfsContainer: apfsContainer ?? self.apfsContainer,
            apfsVolumeInfo: apfsVolumeInfo ?? self.apfsVolumeInfo,
            apfsVolumeRole: apfsVolumeRole ?? self.apfsVolumeRole,
            diskType: diskType ?? self.diskType,
            event: event ?? self.event,
            fileCopyActionOutput: fileCopyActionOutput ?? self.fileCopyActionOutput,
            fileOperation: fileOperation ?? self.fileOperation,
            fileSystem: fileSystem ?? self.fileSystem,
            fsRawEventKind: fsRawEventKind ?? self.fsRawEventKind,
            genericProgress: genericProgress ?? self.genericProgress,
            indexerMetrics: indexerMetrics ?? self.indexerMetrics,
            indexerStats: indexerStats ?? self.indexerStats,
            jobInfoOutput: jobInfoOutput ?? self.jobInfoOutput,
            jobOutput: jobOutput ?? self.jobOutput,
            jobStatus: jobStatus ?? self.jobStatus,
            locationAddOutput: locationAddOutput ?? self.locationAddOutput,
            mountType: mountType ?? self.mountType,
            pathMapping: pathMapping ?? self.pathMapping,
            performanceMetrics: performanceMetrics ?? self.performanceMetrics,
            progress: progress ?? self.progress,
            progressCompletion: progressCompletion ?? self.progressCompletion,
            sdPath: sdPath ?? self.sdPath,
            sdPathBatch: sdPathBatch ?? self.sdPathBatch,
            volume: volume ?? self.volume,
            volumeFingerprint: volumeFingerprint ?? self.volumeFingerprint,
            volumeInfo: volumeInfo ?? self.volumeInfo,
            volumeType: volumeType ?? self.volumeType
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - TypesApfsContainer
struct TypesApfsContainer: Codable {
    let schema: String
    let definitions: ApfsContainerDefinitions
    let description: String
    let properties: ApfsContainerProperties
    let apfsContainerRequired: [String]
    let title: String
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case schema = "$schema"
        case definitions, description, properties
        case apfsContainerRequired = "required"
        case title, type
    }
}

// MARK: TypesApfsContainer convenience initializers and mutators

extension TypesApfsContainer {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(TypesApfsContainer.self, from: data)
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
        schema: String? = nil,
        definitions: ApfsContainerDefinitions? = nil,
        description: String? = nil,
        properties: ApfsContainerProperties? = nil,
        apfsContainerRequired: [String]? = nil,
        title: String? = nil,
        type: OutputType? = nil
    ) -> TypesApfsContainer {
        return TypesApfsContainer(
            schema: schema ?? self.schema,
            definitions: definitions ?? self.definitions,
            description: description ?? self.description,
            properties: properties ?? self.properties,
            apfsContainerRequired: apfsContainerRequired ?? self.apfsContainerRequired,
            title: title ?? self.title,
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

// MARK: - ApfsContainerDefinitions
struct ApfsContainerDefinitions: Codable {
    let apfsVolumeInfo: DefinitionsApfsVolumeInfo
    let apfsVolumeRole: DefinitionsApfsVolumeRole

    enum CodingKeys: String, CodingKey {
        case apfsVolumeInfo = "ApfsVolumeInfo"
        case apfsVolumeRole = "ApfsVolumeRole"
    }
}

// MARK: ApfsContainerDefinitions convenience initializers and mutators

extension ApfsContainerDefinitions {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(ApfsContainerDefinitions.self, from: data)
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
        apfsVolumeInfo: DefinitionsApfsVolumeInfo? = nil,
        apfsVolumeRole: DefinitionsApfsVolumeRole? = nil
    ) -> ApfsContainerDefinitions {
        return ApfsContainerDefinitions(
            apfsVolumeInfo: apfsVolumeInfo ?? self.apfsVolumeInfo,
            apfsVolumeRole: apfsVolumeRole ?? self.apfsVolumeRole
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - DefinitionsApfsVolumeInfo
struct DefinitionsApfsVolumeInfo: Codable {
    let description: String
    let properties: ApfsVolumeInfoProperties
    let apfsVolumeInfoRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case description, properties
        case apfsVolumeInfoRequired = "required"
        case type
    }
}

// MARK: DefinitionsApfsVolumeInfo convenience initializers and mutators

extension DefinitionsApfsVolumeInfo {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(DefinitionsApfsVolumeInfo.self, from: data)
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
        description: String? = nil,
        properties: ApfsVolumeInfoProperties? = nil,
        apfsVolumeInfoRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> DefinitionsApfsVolumeInfo {
        return DefinitionsApfsVolumeInfo(
            description: description ?? self.description,
            properties: properties ?? self.properties,
            apfsVolumeInfoRequired: apfsVolumeInfoRequired ?? self.apfsVolumeInfoRequired,
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

// MARK: - ApfsVolumeInfoProperties
struct ApfsVolumeInfoProperties: Codable {
    let capacityConsumed: CapacityFree
    let diskID, filevault: ContainerID
    let mountPoint: MountPoint
    let name: ContainerID
    let role: Role
    let sealed, uuid: ContainerID

    enum CodingKeys: String, CodingKey {
        case capacityConsumed = "capacity_consumed"
        case diskID = "disk_id"
        case filevault
        case mountPoint = "mount_point"
        case name, role, sealed, uuid
    }
}

// MARK: ApfsVolumeInfoProperties convenience initializers and mutators

extension ApfsVolumeInfoProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(ApfsVolumeInfoProperties.self, from: data)
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
        capacityConsumed: CapacityFree? = nil,
        diskID: ContainerID? = nil,
        filevault: ContainerID? = nil,
        mountPoint: MountPoint? = nil,
        name: ContainerID? = nil,
        role: Role? = nil,
        sealed: ContainerID? = nil,
        uuid: ContainerID? = nil
    ) -> ApfsVolumeInfoProperties {
        return ApfsVolumeInfoProperties(
            capacityConsumed: capacityConsumed ?? self.capacityConsumed,
            diskID: diskID ?? self.diskID,
            filevault: filevault ?? self.filevault,
            mountPoint: mountPoint ?? self.mountPoint,
            name: name ?? self.name,
            role: role ?? self.role,
            sealed: sealed ?? self.sealed,
            uuid: uuid ?? self.uuid
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - ContainerID
struct ContainerID: Codable {
    let description: String
    let type: DestinationType
}

// MARK: ContainerID convenience initializers and mutators

extension ContainerID {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(ContainerID.self, from: data)
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
        description: String? = nil,
        type: DestinationType? = nil
    ) -> ContainerID {
        return ContainerID(
            description: description ?? self.description,
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

// MARK: - MountPoint
struct MountPoint: Codable {
    let description: String
    let type: [ContainerVolumeIDType]
}

// MARK: MountPoint convenience initializers and mutators

extension MountPoint {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(MountPoint.self, from: data)
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
        description: String? = nil,
        type: [ContainerVolumeIDType]? = nil
    ) -> MountPoint {
        return MountPoint(
            description: description ?? self.description,
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

enum ContainerVolumeIDType: String, Codable {
    case null = "null"
    case string = "string"
}

// MARK: - Role
struct Role: Codable {
    let allOf: [ContentDuration]
    let description: String
}

// MARK: Role convenience initializers and mutators

extension Role {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(Role.self, from: data)
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
        allOf: [ContentDuration]? = nil,
        description: String? = nil
    ) -> Role {
        return Role(
            allOf: allOf ?? self.allOf,
            description: description ?? self.description
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - DefinitionsApfsVolumeRole
struct DefinitionsApfsVolumeRole: Codable {
    let description: String
    let oneOf: [ApfsVolumeRoleOneOf]
}

// MARK: DefinitionsApfsVolumeRole convenience initializers and mutators

extension DefinitionsApfsVolumeRole {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(DefinitionsApfsVolumeRole.self, from: data)
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
        description: String? = nil,
        oneOf: [ApfsVolumeRoleOneOf]? = nil
    ) -> DefinitionsApfsVolumeRole {
        return DefinitionsApfsVolumeRole(
            description: description ?? self.description,
            oneOf: oneOf ?? self.oneOf
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - ApfsVolumeRoleOneOf
struct ApfsVolumeRoleOneOf: Codable {
    let description: String
    let oneOfEnum: [String]?
    let type: OutputType
    let additionalProperties: Bool?
    let properties: PurpleProperties?
    let oneOfRequired: [String]?

    enum CodingKeys: String, CodingKey {
        case description
        case oneOfEnum = "enum"
        case type, additionalProperties, properties
        case oneOfRequired = "required"
    }
}

// MARK: ApfsVolumeRoleOneOf convenience initializers and mutators

extension ApfsVolumeRoleOneOf {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(ApfsVolumeRoleOneOf.self, from: data)
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
        description: String? = nil,
        oneOfEnum: [String]?? = nil,
        type: OutputType? = nil,
        additionalProperties: Bool?? = nil,
        properties: PurpleProperties?? = nil,
        oneOfRequired: [String]?? = nil
    ) -> ApfsVolumeRoleOneOf {
        return ApfsVolumeRoleOneOf(
            description: description ?? self.description,
            oneOfEnum: oneOfEnum ?? self.oneOfEnum,
            type: type ?? self.type,
            additionalProperties: additionalProperties ?? self.additionalProperties,
            properties: properties ?? self.properties,
            oneOfRequired: oneOfRequired ?? self.oneOfRequired
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - PurpleProperties
struct PurpleProperties: Codable {
    let other: Destination

    enum CodingKeys: String, CodingKey {
        case other = "Other"
    }
}

// MARK: PurpleProperties convenience initializers and mutators

extension PurpleProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(PurpleProperties.self, from: data)
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
        other: Destination? = nil
    ) -> PurpleProperties {
        return PurpleProperties(
            other: other ?? self.other
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - ApfsContainerProperties
struct ApfsContainerProperties: Codable {
    let capacityFree, capacityInUse: CapacityFree
    let containerID, physicalStore: ContainerID
    let totalCapacity: CapacityFree
    let uuid: ContainerID
    let volumes: Volumes

    enum CodingKeys: String, CodingKey {
        case capacityFree = "capacity_free"
        case capacityInUse = "capacity_in_use"
        case containerID = "container_id"
        case physicalStore = "physical_store"
        case totalCapacity = "total_capacity"
        case uuid, volumes
    }
}

// MARK: ApfsContainerProperties convenience initializers and mutators

extension ApfsContainerProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(ApfsContainerProperties.self, from: data)
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
        capacityFree: CapacityFree? = nil,
        capacityInUse: CapacityFree? = nil,
        containerID: ContainerID? = nil,
        physicalStore: ContainerID? = nil,
        totalCapacity: CapacityFree? = nil,
        uuid: ContainerID? = nil,
        volumes: Volumes? = nil
    ) -> ApfsContainerProperties {
        return ApfsContainerProperties(
            capacityFree: capacityFree ?? self.capacityFree,
            capacityInUse: capacityInUse ?? self.capacityInUse,
            containerID: containerID ?? self.containerID,
            physicalStore: physicalStore ?? self.physicalStore,
            totalCapacity: totalCapacity ?? self.totalCapacity,
            uuid: uuid ?? self.uuid,
            volumes: volumes ?? self.volumes
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - Volumes
struct Volumes: Codable {
    let description: String
    let items: ContentDuration
    let type: String
}

// MARK: Volumes convenience initializers and mutators

extension Volumes {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(Volumes.self, from: data)
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
        description: String? = nil,
        items: ContentDuration? = nil,
        type: String? = nil
    ) -> Volumes {
        return Volumes(
            description: description ?? self.description,
            items: items ?? self.items,
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

// MARK: - TypesApfsVolumeInfo
struct TypesApfsVolumeInfo: Codable {
    let schema: String
    let definitions: ApfsVolumeInfoDefinitions
    let description: String
    let properties: ApfsVolumeInfoProperties
    let apfsVolumeInfoRequired: [String]
    let title: String
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case schema = "$schema"
        case definitions, description, properties
        case apfsVolumeInfoRequired = "required"
        case title, type
    }
}

// MARK: TypesApfsVolumeInfo convenience initializers and mutators

extension TypesApfsVolumeInfo {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(TypesApfsVolumeInfo.self, from: data)
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
        schema: String? = nil,
        definitions: ApfsVolumeInfoDefinitions? = nil,
        description: String? = nil,
        properties: ApfsVolumeInfoProperties? = nil,
        apfsVolumeInfoRequired: [String]? = nil,
        title: String? = nil,
        type: OutputType? = nil
    ) -> TypesApfsVolumeInfo {
        return TypesApfsVolumeInfo(
            schema: schema ?? self.schema,
            definitions: definitions ?? self.definitions,
            description: description ?? self.description,
            properties: properties ?? self.properties,
            apfsVolumeInfoRequired: apfsVolumeInfoRequired ?? self.apfsVolumeInfoRequired,
            title: title ?? self.title,
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

// MARK: - ApfsVolumeInfoDefinitions
struct ApfsVolumeInfoDefinitions: Codable {
    let apfsVolumeRole: DefinitionsApfsVolumeRole

    enum CodingKeys: String, CodingKey {
        case apfsVolumeRole = "ApfsVolumeRole"
    }
}

// MARK: ApfsVolumeInfoDefinitions convenience initializers and mutators

extension ApfsVolumeInfoDefinitions {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(ApfsVolumeInfoDefinitions.self, from: data)
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
        apfsVolumeRole: DefinitionsApfsVolumeRole? = nil
    ) -> ApfsVolumeInfoDefinitions {
        return ApfsVolumeInfoDefinitions(
            apfsVolumeRole: apfsVolumeRole ?? self.apfsVolumeRole
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - TypesApfsVolumeRole
struct TypesApfsVolumeRole: Codable {
    let schema: String
    let description: String
    let oneOf: [ApfsVolumeRoleOneOf]
    let title: String

    enum CodingKeys: String, CodingKey {
        case schema = "$schema"
        case description, oneOf, title
    }
}

// MARK: TypesApfsVolumeRole convenience initializers and mutators

extension TypesApfsVolumeRole {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(TypesApfsVolumeRole.self, from: data)
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
        schema: String? = nil,
        description: String? = nil,
        oneOf: [ApfsVolumeRoleOneOf]? = nil,
        title: String? = nil
    ) -> TypesApfsVolumeRole {
        return TypesApfsVolumeRole(
            schema: schema ?? self.schema,
            description: description ?? self.description,
            oneOf: oneOf ?? self.oneOf,
            title: title ?? self.title
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - TypesDiskType
struct TypesDiskType: Codable {
    let schema: String
    let description: String
    let oneOf: [OneOf]
    let title: String

    enum CodingKeys: String, CodingKey {
        case schema = "$schema"
        case description, oneOf, title
    }
}

// MARK: TypesDiskType convenience initializers and mutators

extension TypesDiskType {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(TypesDiskType.self, from: data)
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
        schema: String? = nil,
        description: String? = nil,
        oneOf: [OneOf]? = nil,
        title: String? = nil
    ) -> TypesDiskType {
        return TypesDiskType(
            schema: schema ?? self.schema,
            description: description ?? self.description,
            oneOf: oneOf ?? self.oneOf,
            title: title ?? self.title
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - OneOf
struct OneOf: Codable {
    let description: String
    let oneOfEnum: [String]
    let type: DestinationType

    enum CodingKeys: String, CodingKey {
        case description
        case oneOfEnum = "enum"
        case type
    }
}

// MARK: OneOf convenience initializers and mutators

extension OneOf {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(OneOf.self, from: data)
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
        description: String? = nil,
        oneOfEnum: [String]? = nil,
        type: DestinationType? = nil
    ) -> OneOf {
        return OneOf(
            description: description ?? self.description,
            oneOfEnum: oneOfEnum ?? self.oneOfEnum,
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

// MARK: - Event
struct Event: Codable {
    let schema: String
    let definitions: EventDefinitions
    let description: String
    let oneOf: [EventOneOf]
    let title: String

    enum CodingKeys: String, CodingKey {
        case schema = "$schema"
        case definitions, description, oneOf, title
    }
}

// MARK: Event convenience initializers and mutators

extension Event {
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

    func with(
        schema: String? = nil,
        definitions: EventDefinitions? = nil,
        description: String? = nil,
        oneOf: [EventOneOf]? = nil,
        title: String? = nil
    ) -> Event {
        return Event(
            schema: schema ?? self.schema,
            definitions: definitions ?? self.definitions,
            description: description ?? self.description,
            oneOf: oneOf ?? self.oneOf,
            title: title ?? self.title
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - EventDefinitions
struct EventDefinitions: Codable {
    let apfsContainer: DefinitionsApfsContainer
    let apfsVolumeInfo: DefinitionsApfsVolumeInfo
    let apfsVolumeRole: DefinitionsApfsVolumeRole
    let diskType: DefinitionsDiskType
    let duration: Duration
    let fileOperation: OneOf
    let fileSystem: DefinitionsApfsVolumeRole
    let fsRawEventKind: DefinitionsFSRawEventKind
    let indexerMetrics: DefinitionsIndexerMetrics
    let indexerStats: DefinitionsIndexerStats
    let jobOutput: DefinitionsJobOutput
    let mountType: DefinitionsDiskType
    let pathMapping: DefinitionsPathMapping
    let volume: DefinitionsVolume
    let volumeFingerprint: ContainerID
    let volumeInfo: DefinitionsVolumeInfo
    let volumeType: DefinitionsDiskType

    enum CodingKeys: String, CodingKey {
        case apfsContainer = "ApfsContainer"
        case apfsVolumeInfo = "ApfsVolumeInfo"
        case apfsVolumeRole = "ApfsVolumeRole"
        case diskType = "DiskType"
        case duration = "Duration"
        case fileOperation = "FileOperation"
        case fileSystem = "FileSystem"
        case fsRawEventKind = "FsRawEventKind"
        case indexerMetrics = "IndexerMetrics"
        case indexerStats = "IndexerStats"
        case jobOutput = "JobOutput"
        case mountType = "MountType"
        case pathMapping = "PathMapping"
        case volume = "Volume"
        case volumeFingerprint = "VolumeFingerprint"
        case volumeInfo = "VolumeInfo"
        case volumeType = "VolumeType"
    }
}

// MARK: EventDefinitions convenience initializers and mutators

extension EventDefinitions {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(EventDefinitions.self, from: data)
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
        apfsContainer: DefinitionsApfsContainer? = nil,
        apfsVolumeInfo: DefinitionsApfsVolumeInfo? = nil,
        apfsVolumeRole: DefinitionsApfsVolumeRole? = nil,
        diskType: DefinitionsDiskType? = nil,
        duration: Duration? = nil,
        fileOperation: OneOf? = nil,
        fileSystem: DefinitionsApfsVolumeRole? = nil,
        fsRawEventKind: DefinitionsFSRawEventKind? = nil,
        indexerMetrics: DefinitionsIndexerMetrics? = nil,
        indexerStats: DefinitionsIndexerStats? = nil,
        jobOutput: DefinitionsJobOutput? = nil,
        mountType: DefinitionsDiskType? = nil,
        pathMapping: DefinitionsPathMapping? = nil,
        volume: DefinitionsVolume? = nil,
        volumeFingerprint: ContainerID? = nil,
        volumeInfo: DefinitionsVolumeInfo? = nil,
        volumeType: DefinitionsDiskType? = nil
    ) -> EventDefinitions {
        return EventDefinitions(
            apfsContainer: apfsContainer ?? self.apfsContainer,
            apfsVolumeInfo: apfsVolumeInfo ?? self.apfsVolumeInfo,
            apfsVolumeRole: apfsVolumeRole ?? self.apfsVolumeRole,
            diskType: diskType ?? self.diskType,
            duration: duration ?? self.duration,
            fileOperation: fileOperation ?? self.fileOperation,
            fileSystem: fileSystem ?? self.fileSystem,
            fsRawEventKind: fsRawEventKind ?? self.fsRawEventKind,
            indexerMetrics: indexerMetrics ?? self.indexerMetrics,
            indexerStats: indexerStats ?? self.indexerStats,
            jobOutput: jobOutput ?? self.jobOutput,
            mountType: mountType ?? self.mountType,
            pathMapping: pathMapping ?? self.pathMapping,
            volume: volume ?? self.volume,
            volumeFingerprint: volumeFingerprint ?? self.volumeFingerprint,
            volumeInfo: volumeInfo ?? self.volumeInfo,
            volumeType: volumeType ?? self.volumeType
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - DefinitionsApfsContainer
struct DefinitionsApfsContainer: Codable {
    let description: String
    let properties: ApfsContainerProperties
    let apfsContainerRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case description, properties
        case apfsContainerRequired = "required"
        case type
    }
}

// MARK: DefinitionsApfsContainer convenience initializers and mutators

extension DefinitionsApfsContainer {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(DefinitionsApfsContainer.self, from: data)
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
        description: String? = nil,
        properties: ApfsContainerProperties? = nil,
        apfsContainerRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> DefinitionsApfsContainer {
        return DefinitionsApfsContainer(
            description: description ?? self.description,
            properties: properties ?? self.properties,
            apfsContainerRequired: apfsContainerRequired ?? self.apfsContainerRequired,
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

// MARK: - DefinitionsDiskType
struct DefinitionsDiskType: Codable {
    let description: String
    let oneOf: [OneOf]
}

// MARK: DefinitionsDiskType convenience initializers and mutators

extension DefinitionsDiskType {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(DefinitionsDiskType.self, from: data)
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
        description: String? = nil,
        oneOf: [OneOf]? = nil
    ) -> DefinitionsDiskType {
        return DefinitionsDiskType(
            description: description ?? self.description,
            oneOf: oneOf ?? self.oneOf
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - Duration
struct Duration: Codable {
    let properties: DurationProperties
    let durationRequired: [DurationRequired]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case durationRequired = "required"
        case type
    }
}

// MARK: Duration convenience initializers and mutators

extension Duration {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(Duration.self, from: data)
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
        properties: DurationProperties? = nil,
        durationRequired: [DurationRequired]? = nil,
        type: OutputType? = nil
    ) -> Duration {
        return Duration(
            properties: properties ?? self.properties,
            durationRequired: durationRequired ?? self.durationRequired,
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

enum DurationRequired: String, Codable {
    case nanos = "nanos"
    case secs = "secs"
}

// MARK: - DurationProperties
struct DurationProperties: Codable {
    let nanos, secs: CapacityFree
}

// MARK: DurationProperties convenience initializers and mutators

extension DurationProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(DurationProperties.self, from: data)
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
        nanos: CapacityFree? = nil,
        secs: CapacityFree? = nil
    ) -> DurationProperties {
        return DurationProperties(
            nanos: nanos ?? self.nanos,
            secs: secs ?? self.secs
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - DefinitionsFSRawEventKind
struct DefinitionsFSRawEventKind: Codable {
    let description: String
    let oneOf: [FSRawEventKindOneOf]
}

// MARK: DefinitionsFSRawEventKind convenience initializers and mutators

extension DefinitionsFSRawEventKind {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(DefinitionsFSRawEventKind.self, from: data)
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
        description: String? = nil,
        oneOf: [FSRawEventKindOneOf]? = nil
    ) -> DefinitionsFSRawEventKind {
        return DefinitionsFSRawEventKind(
            description: description ?? self.description,
            oneOf: oneOf ?? self.oneOf
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - FSRawEventKindOneOf
struct FSRawEventKindOneOf: Codable {
    let additionalProperties: Bool
    let properties: FluffyProperties
    let oneOfRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case additionalProperties, properties
        case oneOfRequired = "required"
        case type
    }
}

// MARK: FSRawEventKindOneOf convenience initializers and mutators

extension FSRawEventKindOneOf {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(FSRawEventKindOneOf.self, from: data)
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
        additionalProperties: Bool? = nil,
        properties: FluffyProperties? = nil,
        oneOfRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> FSRawEventKindOneOf {
        return FSRawEventKindOneOf(
            additionalProperties: additionalProperties ?? self.additionalProperties,
            properties: properties ?? self.properties,
            oneOfRequired: oneOfRequired ?? self.oneOfRequired,
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

// MARK: - FluffyProperties
struct FluffyProperties: Codable {
    let create, modify, remove: Create?
    let rename: Rename?

    enum CodingKeys: String, CodingKey {
        case create = "Create"
        case modify = "Modify"
        case remove = "Remove"
        case rename = "Rename"
    }
}

// MARK: FluffyProperties convenience initializers and mutators

extension FluffyProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(FluffyProperties.self, from: data)
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
        create: Create?? = nil,
        modify: Create?? = nil,
        remove: Create?? = nil,
        rename: Rename?? = nil
    ) -> FluffyProperties {
        return FluffyProperties(
            create: create ?? self.create,
            modify: modify ?? self.modify,
            remove: remove ?? self.remove,
            rename: rename ?? self.rename
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
struct Create: Codable {
    let properties: CreateProperties
    let createRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case createRequired = "required"
        case type
    }
}

// MARK: Create convenience initializers and mutators

extension Create {
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
        properties: CreateProperties? = nil,
        createRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> Create {
        return Create(
            properties: properties ?? self.properties,
            createRequired: createRequired ?? self.createRequired,
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

// MARK: - CreateProperties
struct CreateProperties: Codable {
    let path: Destination
}

// MARK: CreateProperties convenience initializers and mutators

extension CreateProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(CreateProperties.self, from: data)
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
        path: Destination? = nil
    ) -> CreateProperties {
        return CreateProperties(
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

// MARK: - Rename
struct Rename: Codable {
    let properties: RenameProperties
    let renameRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case renameRequired = "required"
        case type
    }
}

// MARK: Rename convenience initializers and mutators

extension Rename {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(Rename.self, from: data)
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
        properties: RenameProperties? = nil,
        renameRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> Rename {
        return Rename(
            properties: properties ?? self.properties,
            renameRequired: renameRequired ?? self.renameRequired,
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

// MARK: - RenameProperties
struct RenameProperties: Codable {
    let from, to: Destination
}

// MARK: RenameProperties convenience initializers and mutators

extension RenameProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(RenameProperties.self, from: data)
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
        from: Destination? = nil,
        to: Destination? = nil
    ) -> RenameProperties {
        return RenameProperties(
            from: from ?? self.from,
            to: to ?? self.to
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - DefinitionsIndexerMetrics
struct DefinitionsIndexerMetrics: Codable {
    let description: String
    let properties: IndexerMetricsProperties
    let indexerMetricsRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case description, properties
        case indexerMetricsRequired = "required"
        case type
    }
}

// MARK: DefinitionsIndexerMetrics convenience initializers and mutators

extension DefinitionsIndexerMetrics {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(DefinitionsIndexerMetrics.self, from: data)
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
        description: String? = nil,
        properties: IndexerMetricsProperties? = nil,
        indexerMetricsRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> DefinitionsIndexerMetrics {
        return DefinitionsIndexerMetrics(
            description: description ?? self.description,
            properties: properties ?? self.properties,
            indexerMetricsRequired: indexerMetricsRequired ?? self.indexerMetricsRequired,
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

// MARK: - IndexerMetricsProperties
struct IndexerMetricsProperties: Codable {
    let avgBatchSize: JobID
    let avgMemoryBytes: AvgMemoryBytes
    let batchCount: CapacityFree
    let bytesPerSecond: JobID
    let contentDuration: ContentDuration
    let criticalErrors, dbReads, dbWrites: CapacityFree
    let dirsPerSecond: JobID
    let discoveryDuration: ContentDuration
    let filesPerSecond: JobID
    let nonCriticalErrors: CapacityFree
    let peakMemoryBytes: AvgMemoryBytes
    let processingDuration: ContentDuration
    let skippedPaths: CapacityFree
    let totalDuration: ContentDuration
    let totalErrors: CapacityFree

    enum CodingKeys: String, CodingKey {
        case avgBatchSize = "avg_batch_size"
        case avgMemoryBytes = "avg_memory_bytes"
        case batchCount = "batch_count"
        case bytesPerSecond = "bytes_per_second"
        case contentDuration = "content_duration"
        case criticalErrors = "critical_errors"
        case dbReads = "db_reads"
        case dbWrites = "db_writes"
        case dirsPerSecond = "dirs_per_second"
        case discoveryDuration = "discovery_duration"
        case filesPerSecond = "files_per_second"
        case nonCriticalErrors = "non_critical_errors"
        case peakMemoryBytes = "peak_memory_bytes"
        case processingDuration = "processing_duration"
        case skippedPaths = "skipped_paths"
        case totalDuration = "total_duration"
        case totalErrors = "total_errors"
    }
}

// MARK: IndexerMetricsProperties convenience initializers and mutators

extension IndexerMetricsProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(IndexerMetricsProperties.self, from: data)
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
        avgBatchSize: JobID? = nil,
        avgMemoryBytes: AvgMemoryBytes? = nil,
        batchCount: CapacityFree? = nil,
        bytesPerSecond: JobID? = nil,
        contentDuration: ContentDuration? = nil,
        criticalErrors: CapacityFree? = nil,
        dbReads: CapacityFree? = nil,
        dbWrites: CapacityFree? = nil,
        dirsPerSecond: JobID? = nil,
        discoveryDuration: ContentDuration? = nil,
        filesPerSecond: JobID? = nil,
        nonCriticalErrors: CapacityFree? = nil,
        peakMemoryBytes: AvgMemoryBytes? = nil,
        processingDuration: ContentDuration? = nil,
        skippedPaths: CapacityFree? = nil,
        totalDuration: ContentDuration? = nil,
        totalErrors: CapacityFree? = nil
    ) -> IndexerMetricsProperties {
        return IndexerMetricsProperties(
            avgBatchSize: avgBatchSize ?? self.avgBatchSize,
            avgMemoryBytes: avgMemoryBytes ?? self.avgMemoryBytes,
            batchCount: batchCount ?? self.batchCount,
            bytesPerSecond: bytesPerSecond ?? self.bytesPerSecond,
            contentDuration: contentDuration ?? self.contentDuration,
            criticalErrors: criticalErrors ?? self.criticalErrors,
            dbReads: dbReads ?? self.dbReads,
            dbWrites: dbWrites ?? self.dbWrites,
            dirsPerSecond: dirsPerSecond ?? self.dirsPerSecond,
            discoveryDuration: discoveryDuration ?? self.discoveryDuration,
            filesPerSecond: filesPerSecond ?? self.filesPerSecond,
            nonCriticalErrors: nonCriticalErrors ?? self.nonCriticalErrors,
            peakMemoryBytes: peakMemoryBytes ?? self.peakMemoryBytes,
            processingDuration: processingDuration ?? self.processingDuration,
            skippedPaths: skippedPaths ?? self.skippedPaths,
            totalDuration: totalDuration ?? self.totalDuration,
            totalErrors: totalErrors ?? self.totalErrors
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - JobID
struct JobID: Codable {
    let format: Format
    let type: CapacityFreeType
}

// MARK: JobID convenience initializers and mutators

extension JobID {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(JobID.self, from: data)
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
        format: Format? = nil,
        type: CapacityFreeType? = nil
    ) -> JobID {
        return JobID(
            format: format ?? self.format,
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

// MARK: - AvgMemoryBytes
struct AvgMemoryBytes: Codable {
    let format: Format
    let minimum: Double
    let type: [AvgMemoryBytesType]
    let description: String?
}

// MARK: AvgMemoryBytes convenience initializers and mutators

extension AvgMemoryBytes {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(AvgMemoryBytes.self, from: data)
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
        format: Format? = nil,
        minimum: Double? = nil,
        type: [AvgMemoryBytesType]? = nil,
        description: String?? = nil
    ) -> AvgMemoryBytes {
        return AvgMemoryBytes(
            format: format ?? self.format,
            minimum: minimum ?? self.minimum,
            type: type ?? self.type,
            description: description ?? self.description
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

enum AvgMemoryBytesType: String, Codable {
    case integer = "integer"
    case null = "null"
}

// MARK: - DefinitionsIndexerStats
struct DefinitionsIndexerStats: Codable {
    let description: String
    let properties: IndexerStatsProperties
    let indexerStatsRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case description, properties
        case indexerStatsRequired = "required"
        case type
    }
}

// MARK: DefinitionsIndexerStats convenience initializers and mutators

extension DefinitionsIndexerStats {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(DefinitionsIndexerStats.self, from: data)
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
        description: String? = nil,
        properties: IndexerStatsProperties? = nil,
        indexerStatsRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> DefinitionsIndexerStats {
        return DefinitionsIndexerStats(
            description: description ?? self.description,
            properties: properties ?? self.properties,
            indexerStatsRequired: indexerStatsRequired ?? self.indexerStatsRequired,
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

// MARK: - IndexerStatsProperties
struct IndexerStatsProperties: Codable {
    let bytes, dirs, errors, files: CapacityFree
    let skipped, symlinks: CapacityFree
}

// MARK: IndexerStatsProperties convenience initializers and mutators

extension IndexerStatsProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(IndexerStatsProperties.self, from: data)
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
        bytes: CapacityFree? = nil,
        dirs: CapacityFree? = nil,
        errors: CapacityFree? = nil,
        files: CapacityFree? = nil,
        skipped: CapacityFree? = nil,
        symlinks: CapacityFree? = nil
    ) -> IndexerStatsProperties {
        return IndexerStatsProperties(
            bytes: bytes ?? self.bytes,
            dirs: dirs ?? self.dirs,
            errors: errors ?? self.errors,
            files: files ?? self.files,
            skipped: skipped ?? self.skipped,
            symlinks: symlinks ?? self.symlinks
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - DefinitionsJobOutput
struct DefinitionsJobOutput: Codable {
    let description: String
    let oneOf: [JobOutputOneOf]
}

// MARK: DefinitionsJobOutput convenience initializers and mutators

extension DefinitionsJobOutput {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(DefinitionsJobOutput.self, from: data)
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
        description: String? = nil,
        oneOf: [JobOutputOneOf]? = nil
    ) -> DefinitionsJobOutput {
        return DefinitionsJobOutput(
            description: description ?? self.description,
            oneOf: oneOf ?? self.oneOf
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - JobOutputOneOf
struct JobOutputOneOf: Codable {
    let description: String
    let properties: TentacledProperties
    let oneOfRequired: [OneOfRequired]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case description, properties
        case oneOfRequired = "required"
        case type
    }
}

// MARK: JobOutputOneOf convenience initializers and mutators

extension JobOutputOneOf {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(JobOutputOneOf.self, from: data)
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
        description: String? = nil,
        properties: TentacledProperties? = nil,
        oneOfRequired: [OneOfRequired]? = nil,
        type: OutputType? = nil
    ) -> JobOutputOneOf {
        return JobOutputOneOf(
            description: description ?? self.description,
            properties: properties ?? self.properties,
            oneOfRequired: oneOfRequired ?? self.oneOfRequired,
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

enum OneOfRequired: String, Codable {
    case data = "data"
    case type = "type"
}

// MARK: - TentacledProperties
struct TentacledProperties: Codable {
    let type: TypeClass
    let data: TentacledData?
}

// MARK: TentacledProperties convenience initializers and mutators

extension TentacledProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(TentacledProperties.self, from: data)
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
        type: TypeClass? = nil,
        data: TentacledData?? = nil
    ) -> TentacledProperties {
        return TentacledProperties(
            type: type ?? self.type,
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

enum TentacledData: Codable {
    case bool(Bool)
    case purpleData(PurpleData)

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let x = try? container.decode(Bool.self) {
            self = .bool(x)
            return
        }
        if let x = try? container.decode(PurpleData.self) {
            self = .purpleData(x)
            return
        }
        throw DecodingError.typeMismatch(TentacledData.self, DecodingError.Context(codingPath: decoder.codingPath, debugDescription: "Wrong type for TentacledData"))
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .bool(let x):
            try container.encode(x)
        case .purpleData(let x):
            try container.encode(x)
        }
    }
}

// MARK: - PurpleData
struct PurpleData: Codable {
    let properties: StickyProperties
    let dataRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case dataRequired = "required"
        case type
    }
}

// MARK: PurpleData convenience initializers and mutators

extension PurpleData {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(PurpleData.self, from: data)
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
        properties: StickyProperties? = nil,
        dataRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> PurpleData {
        return PurpleData(
            properties: properties ?? self.properties,
            dataRequired: dataRequired ?? self.dataRequired,
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

// MARK: - StickyProperties
struct StickyProperties: Codable {
    let copiedCount, totalBytes: CapacityFree?
    let metrics, stats: ContentDuration?
    let failedCount, generatedCount, errorCount, skippedCount: CapacityFree?
    let totalSizeBytes, movedCount, deletedCount, duplicateGroups: CapacityFree?
    let potentialSavings, totalDuplicates, issuesFound, totalBytesValidated: CapacityFree?
    let validatedCount: CapacityFree?

    enum CodingKeys: String, CodingKey {
        case copiedCount = "copied_count"
        case totalBytes = "total_bytes"
        case metrics, stats
        case failedCount = "failed_count"
        case generatedCount = "generated_count"
        case errorCount = "error_count"
        case skippedCount = "skipped_count"
        case totalSizeBytes = "total_size_bytes"
        case movedCount = "moved_count"
        case deletedCount = "deleted_count"
        case duplicateGroups = "duplicate_groups"
        case potentialSavings = "potential_savings"
        case totalDuplicates = "total_duplicates"
        case issuesFound = "issues_found"
        case totalBytesValidated = "total_bytes_validated"
        case validatedCount = "validated_count"
    }
}

// MARK: StickyProperties convenience initializers and mutators

extension StickyProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(StickyProperties.self, from: data)
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
        copiedCount: CapacityFree?? = nil,
        totalBytes: CapacityFree?? = nil,
        metrics: ContentDuration?? = nil,
        stats: ContentDuration?? = nil,
        failedCount: CapacityFree?? = nil,
        generatedCount: CapacityFree?? = nil,
        errorCount: CapacityFree?? = nil,
        skippedCount: CapacityFree?? = nil,
        totalSizeBytes: CapacityFree?? = nil,
        movedCount: CapacityFree?? = nil,
        deletedCount: CapacityFree?? = nil,
        duplicateGroups: CapacityFree?? = nil,
        potentialSavings: CapacityFree?? = nil,
        totalDuplicates: CapacityFree?? = nil,
        issuesFound: CapacityFree?? = nil,
        totalBytesValidated: CapacityFree?? = nil,
        validatedCount: CapacityFree?? = nil
    ) -> StickyProperties {
        return StickyProperties(
            copiedCount: copiedCount ?? self.copiedCount,
            totalBytes: totalBytes ?? self.totalBytes,
            metrics: metrics ?? self.metrics,
            stats: stats ?? self.stats,
            failedCount: failedCount ?? self.failedCount,
            generatedCount: generatedCount ?? self.generatedCount,
            errorCount: errorCount ?? self.errorCount,
            skippedCount: skippedCount ?? self.skippedCount,
            totalSizeBytes: totalSizeBytes ?? self.totalSizeBytes,
            movedCount: movedCount ?? self.movedCount,
            deletedCount: deletedCount ?? self.deletedCount,
            duplicateGroups: duplicateGroups ?? self.duplicateGroups,
            potentialSavings: potentialSavings ?? self.potentialSavings,
            totalDuplicates: totalDuplicates ?? self.totalDuplicates,
            issuesFound: issuesFound ?? self.issuesFound,
            totalBytesValidated: totalBytesValidated ?? self.totalBytesValidated,
            validatedCount: validatedCount ?? self.validatedCount
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - TypeClass
struct TypeClass: Codable {
    let typeEnum: [String]
    let type: DestinationType

    enum CodingKeys: String, CodingKey {
        case typeEnum = "enum"
        case type
    }
}

// MARK: TypeClass convenience initializers and mutators

extension TypeClass {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(TypeClass.self, from: data)
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
        typeEnum: [String]? = nil,
        type: DestinationType? = nil
    ) -> TypeClass {
        return TypeClass(
            typeEnum: typeEnum ?? self.typeEnum,
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

// MARK: - DefinitionsPathMapping
struct DefinitionsPathMapping: Codable {
    let description: String
    let properties: PathMappingProperties
    let pathMappingRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case description, properties
        case pathMappingRequired = "required"
        case type
    }
}

// MARK: DefinitionsPathMapping convenience initializers and mutators

extension DefinitionsPathMapping {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(DefinitionsPathMapping.self, from: data)
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
        description: String? = nil,
        properties: PathMappingProperties? = nil,
        pathMappingRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> DefinitionsPathMapping {
        return DefinitionsPathMapping(
            description: description ?? self.description,
            properties: properties ?? self.properties,
            pathMappingRequired: pathMappingRequired ?? self.pathMappingRequired,
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

// MARK: - PathMappingProperties
struct PathMappingProperties: Codable {
    let actualPath, virtualPath: ContainerID

    enum CodingKeys: String, CodingKey {
        case actualPath = "actual_path"
        case virtualPath = "virtual_path"
    }
}

// MARK: PathMappingProperties convenience initializers and mutators

extension PathMappingProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(PathMappingProperties.self, from: data)
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
        actualPath: ContainerID? = nil,
        virtualPath: ContainerID? = nil
    ) -> PathMappingProperties {
        return PathMappingProperties(
            actualPath: actualPath ?? self.actualPath,
            virtualPath: virtualPath ?? self.virtualPath
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - DefinitionsVolume
struct DefinitionsVolume: Codable {
    let description: String
    let properties: VolumeProperties
    let volumeRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case description, properties
        case volumeRequired = "required"
        case type
    }
}

// MARK: DefinitionsVolume convenience initializers and mutators

extension DefinitionsVolume {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(DefinitionsVolume.self, from: data)
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
        description: String? = nil,
        properties: VolumeProperties? = nil,
        volumeRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> DefinitionsVolume {
        return DefinitionsVolume(
            description: description ?? self.description,
            properties: properties ?? self.properties,
            volumeRequired: volumeRequired ?? self.volumeRequired,
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

// MARK: - VolumeProperties
struct VolumeProperties: Codable {
    let apfsContainer: CurrentPath
    let autoTrackEligible: ContainerID
    let containerVolumeID: MountPoint
    let deviceID: CapacityFree
    let diskType: Role
    let errorStatus: MountPoint
    let fileSystem, fingerprint: Role
    let hardwareID: MountPoint
    let isMounted, isUserVisible: ContainerID
    let lastUpdated: CapacityFree
    let mountPoint: ContainerID
    let mountPoints: MountPoints
    let mountType: Role
    let name: ContainerID
    let pathMappings: Volumes
    let readOnly: ContainerID
    let readSpeedMbps: AvgMemoryBytes
    let totalBytesAvailable, totalBytesCapacity: CapacityFree
    let volumeType: Role
    let writeSpeedMbps: AvgMemoryBytes

    enum CodingKeys: String, CodingKey {
        case apfsContainer = "apfs_container"
        case autoTrackEligible = "auto_track_eligible"
        case containerVolumeID = "container_volume_id"
        case deviceID = "device_id"
        case diskType = "disk_type"
        case errorStatus = "error_status"
        case fileSystem = "file_system"
        case fingerprint
        case hardwareID = "hardware_id"
        case isMounted = "is_mounted"
        case isUserVisible = "is_user_visible"
        case lastUpdated = "last_updated"
        case mountPoint = "mount_point"
        case mountPoints = "mount_points"
        case mountType = "mount_type"
        case name
        case pathMappings = "path_mappings"
        case readOnly = "read_only"
        case readSpeedMbps = "read_speed_mbps"
        case totalBytesAvailable = "total_bytes_available"
        case totalBytesCapacity = "total_bytes_capacity"
        case volumeType = "volume_type"
        case writeSpeedMbps = "write_speed_mbps"
    }
}

// MARK: VolumeProperties convenience initializers and mutators

extension VolumeProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(VolumeProperties.self, from: data)
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
        apfsContainer: CurrentPath? = nil,
        autoTrackEligible: ContainerID? = nil,
        containerVolumeID: MountPoint? = nil,
        deviceID: CapacityFree? = nil,
        diskType: Role? = nil,
        errorStatus: MountPoint? = nil,
        fileSystem: Role? = nil,
        fingerprint: Role? = nil,
        hardwareID: MountPoint? = nil,
        isMounted: ContainerID? = nil,
        isUserVisible: ContainerID? = nil,
        lastUpdated: CapacityFree? = nil,
        mountPoint: ContainerID? = nil,
        mountPoints: MountPoints? = nil,
        mountType: Role? = nil,
        name: ContainerID? = nil,
        pathMappings: Volumes? = nil,
        readOnly: ContainerID? = nil,
        readSpeedMbps: AvgMemoryBytes? = nil,
        totalBytesAvailable: CapacityFree? = nil,
        totalBytesCapacity: CapacityFree? = nil,
        volumeType: Role? = nil,
        writeSpeedMbps: AvgMemoryBytes? = nil
    ) -> VolumeProperties {
        return VolumeProperties(
            apfsContainer: apfsContainer ?? self.apfsContainer,
            autoTrackEligible: autoTrackEligible ?? self.autoTrackEligible,
            containerVolumeID: containerVolumeID ?? self.containerVolumeID,
            deviceID: deviceID ?? self.deviceID,
            diskType: diskType ?? self.diskType,
            errorStatus: errorStatus ?? self.errorStatus,
            fileSystem: fileSystem ?? self.fileSystem,
            fingerprint: fingerprint ?? self.fingerprint,
            hardwareID: hardwareID ?? self.hardwareID,
            isMounted: isMounted ?? self.isMounted,
            isUserVisible: isUserVisible ?? self.isUserVisible,
            lastUpdated: lastUpdated ?? self.lastUpdated,
            mountPoint: mountPoint ?? self.mountPoint,
            mountPoints: mountPoints ?? self.mountPoints,
            mountType: mountType ?? self.mountType,
            name: name ?? self.name,
            pathMappings: pathMappings ?? self.pathMappings,
            readOnly: readOnly ?? self.readOnly,
            readSpeedMbps: readSpeedMbps ?? self.readSpeedMbps,
            totalBytesAvailable: totalBytesAvailable ?? self.totalBytesAvailable,
            totalBytesCapacity: totalBytesCapacity ?? self.totalBytesCapacity,
            volumeType: volumeType ?? self.volumeType,
            writeSpeedMbps: writeSpeedMbps ?? self.writeSpeedMbps
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - CurrentPath
struct CurrentPath: Codable {
    let anyOf: [AnyOf]
    let description: String
}

// MARK: CurrentPath convenience initializers and mutators

extension CurrentPath {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(CurrentPath.self, from: data)
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
        anyOf: [AnyOf]? = nil,
        description: String? = nil
    ) -> CurrentPath {
        return CurrentPath(
            anyOf: anyOf ?? self.anyOf,
            description: description ?? self.description
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - AnyOf
struct AnyOf: Codable {
    let ref: Ref?
    let type: ContainerVolumeIDType?

    enum CodingKeys: String, CodingKey {
        case ref = "$ref"
        case type
    }
}

// MARK: AnyOf convenience initializers and mutators

extension AnyOf {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(AnyOf.self, from: data)
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
        ref: Ref?? = nil,
        type: ContainerVolumeIDType?? = nil
    ) -> AnyOf {
        return AnyOf(
            ref: ref ?? self.ref,
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

enum Ref: String, Codable {
    case definitionsApfsContainer = "#/definitions/ApfsContainer"
    case definitionsDuration = "#/definitions/Duration"
    case definitionsSDPath = "#/definitions/SdPath"
}

// MARK: - MountPoints
struct MountPoints: Codable {
    let description: String
    let items: Destination
    let type: String
}

// MARK: MountPoints convenience initializers and mutators

extension MountPoints {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(MountPoints.self, from: data)
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
        description: String? = nil,
        items: Destination? = nil,
        type: String? = nil
    ) -> MountPoints {
        return MountPoints(
            description: description ?? self.description,
            items: items ?? self.items,
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

// MARK: - DefinitionsVolumeInfo
struct DefinitionsVolumeInfo: Codable {
    let description: String
    let properties: VolumeInfoProperties
    let volumeInfoRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case description, properties
        case volumeInfoRequired = "required"
        case type
    }
}

// MARK: DefinitionsVolumeInfo convenience initializers and mutators

extension DefinitionsVolumeInfo {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(DefinitionsVolumeInfo.self, from: data)
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
        description: String? = nil,
        properties: VolumeInfoProperties? = nil,
        volumeInfoRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> DefinitionsVolumeInfo {
        return DefinitionsVolumeInfo(
            description: description ?? self.description,
            properties: properties ?? self.properties,
            volumeInfoRequired: volumeInfoRequired ?? self.volumeInfoRequired,
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

// MARK: - VolumeInfoProperties
struct VolumeInfoProperties: Codable {
    let errorStatus: ErrorMessage
    let isMounted: Destination
    let readSpeedMbps: AvgMemoryBytes
    let totalBytesAvailable: CapacityFree
    let writeSpeedMbps: AvgMemoryBytes

    enum CodingKeys: String, CodingKey {
        case errorStatus = "error_status"
        case isMounted = "is_mounted"
        case readSpeedMbps = "read_speed_mbps"
        case totalBytesAvailable = "total_bytes_available"
        case writeSpeedMbps = "write_speed_mbps"
    }
}

// MARK: VolumeInfoProperties convenience initializers and mutators

extension VolumeInfoProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(VolumeInfoProperties.self, from: data)
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
        errorStatus: ErrorMessage? = nil,
        isMounted: Destination? = nil,
        readSpeedMbps: AvgMemoryBytes? = nil,
        totalBytesAvailable: CapacityFree? = nil,
        writeSpeedMbps: AvgMemoryBytes? = nil
    ) -> VolumeInfoProperties {
        return VolumeInfoProperties(
            errorStatus: errorStatus ?? self.errorStatus,
            isMounted: isMounted ?? self.isMounted,
            readSpeedMbps: readSpeedMbps ?? self.readSpeedMbps,
            totalBytesAvailable: totalBytesAvailable ?? self.totalBytesAvailable,
            writeSpeedMbps: writeSpeedMbps ?? self.writeSpeedMbps
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - ErrorMessage
struct ErrorMessage: Codable {
    let type: [ContainerVolumeIDType]
}

// MARK: ErrorMessage convenience initializers and mutators

extension ErrorMessage {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(ErrorMessage.self, from: data)
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
        type: [ContainerVolumeIDType]? = nil
    ) -> ErrorMessage {
        return ErrorMessage(
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

// MARK: - EventOneOf
struct EventOneOf: Codable {
    let oneOfEnum: [String]?
    let type: OutputType
    let additionalProperties: Bool?
    let properties: IndigoProperties?
    let oneOfRequired: [String]?

    enum CodingKeys: String, CodingKey {
        case oneOfEnum = "enum"
        case type, additionalProperties, properties
        case oneOfRequired = "required"
    }
}

// MARK: EventOneOf convenience initializers and mutators

extension EventOneOf {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(EventOneOf.self, from: data)
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
        oneOfEnum: [String]?? = nil,
        type: OutputType? = nil,
        additionalProperties: Bool?? = nil,
        properties: IndigoProperties?? = nil,
        oneOfRequired: [String]?? = nil
    ) -> EventOneOf {
        return EventOneOf(
            oneOfEnum: oneOfEnum ?? self.oneOfEnum,
            type: type ?? self.type,
            additionalProperties: additionalProperties ?? self.additionalProperties,
            properties: properties ?? self.properties,
            oneOfRequired: oneOfRequired ?? self.oneOfRequired
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - IndigoProperties
struct IndigoProperties: Codable {
    let libraryCreated, libraryOpened: Library?
    let libraryClosed: LibraryClosed?
    let libraryDeleted: LibraryDeleted?
    let entryCreated, entryModified, entryDeleted: Entry?
    let entryMoved: EntryMoved?
    let fsRawChange: FSRawChange?
    let volumeAdded: ContentDuration?
    let volumeRemoved: VolumeRemoved?
    let volumeUpdated: VolumeUpdated?
    let volumeSpeedTested: VolumeSpeedTested?
    let volumeMountChanged: VolumeMountChanged?
    let volumeError: VolumeError?
    let jobQueued, jobStarted: JobCancelledClass?
    let jobProgress: JobProgress?
    let jobCompleted: JobCompleted?
    let jobFailed: JobFailed?
    let jobCancelled: JobCancelledClass?
    let jobPaused, jobResumed: JobPausedClass?
    let indexingStarted: IndexingStarted?
    let indexingProgress: IndexingProgress?
    let indexingCompleted: IndexingCompleted?
    let indexingFailed: IndexingFailed?
    let deviceConnected: DeviceConnected?
    let deviceDisconnected: DeviceDisconnected?
    let locationAdded: LocationAdded?
    let locationRemoved: LocationRemoved?
    let filesIndexed: FilesIndexed?
    let thumbnailsGenerated: ThumbnailsGenerated?
    let fileOperationCompleted: FileOperationCompleted?
    let filesModified: FilesModified?
    let logMessage: LogMessage?
    let custom: Custom?

    enum CodingKeys: String, CodingKey {
        case libraryCreated = "LibraryCreated"
        case libraryOpened = "LibraryOpened"
        case libraryClosed = "LibraryClosed"
        case libraryDeleted = "LibraryDeleted"
        case entryCreated = "EntryCreated"
        case entryModified = "EntryModified"
        case entryDeleted = "EntryDeleted"
        case entryMoved = "EntryMoved"
        case fsRawChange = "FsRawChange"
        case volumeAdded = "VolumeAdded"
        case volumeRemoved = "VolumeRemoved"
        case volumeUpdated = "VolumeUpdated"
        case volumeSpeedTested = "VolumeSpeedTested"
        case volumeMountChanged = "VolumeMountChanged"
        case volumeError = "VolumeError"
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
        case locationAdded = "LocationAdded"
        case locationRemoved = "LocationRemoved"
        case filesIndexed = "FilesIndexed"
        case thumbnailsGenerated = "ThumbnailsGenerated"
        case fileOperationCompleted = "FileOperationCompleted"
        case filesModified = "FilesModified"
        case logMessage = "LogMessage"
        case custom = "Custom"
    }
}

// MARK: IndigoProperties convenience initializers and mutators

extension IndigoProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(IndigoProperties.self, from: data)
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
        fsRawChange: FSRawChange?? = nil,
        volumeAdded: ContentDuration?? = nil,
        volumeRemoved: VolumeRemoved?? = nil,
        volumeUpdated: VolumeUpdated?? = nil,
        volumeSpeedTested: VolumeSpeedTested?? = nil,
        volumeMountChanged: VolumeMountChanged?? = nil,
        volumeError: VolumeError?? = nil,
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
        locationAdded: LocationAdded?? = nil,
        locationRemoved: LocationRemoved?? = nil,
        filesIndexed: FilesIndexed?? = nil,
        thumbnailsGenerated: ThumbnailsGenerated?? = nil,
        fileOperationCompleted: FileOperationCompleted?? = nil,
        filesModified: FilesModified?? = nil,
        logMessage: LogMessage?? = nil,
        custom: Custom?? = nil
    ) -> IndigoProperties {
        return IndigoProperties(
            libraryCreated: libraryCreated ?? self.libraryCreated,
            libraryOpened: libraryOpened ?? self.libraryOpened,
            libraryClosed: libraryClosed ?? self.libraryClosed,
            libraryDeleted: libraryDeleted ?? self.libraryDeleted,
            entryCreated: entryCreated ?? self.entryCreated,
            entryModified: entryModified ?? self.entryModified,
            entryDeleted: entryDeleted ?? self.entryDeleted,
            entryMoved: entryMoved ?? self.entryMoved,
            fsRawChange: fsRawChange ?? self.fsRawChange,
            volumeAdded: volumeAdded ?? self.volumeAdded,
            volumeRemoved: volumeRemoved ?? self.volumeRemoved,
            volumeUpdated: volumeUpdated ?? self.volumeUpdated,
            volumeSpeedTested: volumeSpeedTested ?? self.volumeSpeedTested,
            volumeMountChanged: volumeMountChanged ?? self.volumeMountChanged,
            volumeError: volumeError ?? self.volumeError,
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
struct Custom: Codable {
    let properties: CustomProperties
    let customRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case customRequired = "required"
        case type
    }
}

// MARK: Custom convenience initializers and mutators

extension Custom {
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
        properties: CustomProperties? = nil,
        customRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> Custom {
        return Custom(
            properties: properties ?? self.properties,
            customRequired: customRequired ?? self.customRequired,
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

// MARK: - CustomProperties
struct CustomProperties: Codable {
    let data: Bool
    let eventType: Destination

    enum CodingKeys: String, CodingKey {
        case data
        case eventType = "event_type"
    }
}

// MARK: CustomProperties convenience initializers and mutators

extension CustomProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(CustomProperties.self, from: data)
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
        data: Bool? = nil,
        eventType: Destination? = nil
    ) -> CustomProperties {
        return CustomProperties(
            data: data ?? self.data,
            eventType: eventType ?? self.eventType
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
struct DeviceConnected: Codable {
    let properties: DeviceConnectedProperties
    let deviceConnectedRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case deviceConnectedRequired = "required"
        case type
    }
}

// MARK: DeviceConnected convenience initializers and mutators

extension DeviceConnected {
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
        properties: DeviceConnectedProperties? = nil,
        deviceConnectedRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> DeviceConnected {
        return DeviceConnected(
            properties: properties ?? self.properties,
            deviceConnectedRequired: deviceConnectedRequired ?? self.deviceConnectedRequired,
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

// MARK: - DeviceConnectedProperties
struct DeviceConnectedProperties: Codable {
    let deviceID: JobID
    let deviceName: Destination

    enum CodingKeys: String, CodingKey {
        case deviceID = "device_id"
        case deviceName = "device_name"
    }
}

// MARK: DeviceConnectedProperties convenience initializers and mutators

extension DeviceConnectedProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(DeviceConnectedProperties.self, from: data)
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
        deviceID: JobID? = nil,
        deviceName: Destination? = nil
    ) -> DeviceConnectedProperties {
        return DeviceConnectedProperties(
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
struct DeviceDisconnected: Codable {
    let properties: DeviceDisconnectedProperties
    let deviceDisconnectedRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case deviceDisconnectedRequired = "required"
        case type
    }
}

// MARK: DeviceDisconnected convenience initializers and mutators

extension DeviceDisconnected {
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
        properties: DeviceDisconnectedProperties? = nil,
        deviceDisconnectedRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> DeviceDisconnected {
        return DeviceDisconnected(
            properties: properties ?? self.properties,
            deviceDisconnectedRequired: deviceDisconnectedRequired ?? self.deviceDisconnectedRequired,
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

// MARK: - DeviceDisconnectedProperties
struct DeviceDisconnectedProperties: Codable {
    let deviceID: JobID

    enum CodingKeys: String, CodingKey {
        case deviceID = "device_id"
    }
}

// MARK: DeviceDisconnectedProperties convenience initializers and mutators

extension DeviceDisconnectedProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(DeviceDisconnectedProperties.self, from: data)
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
        deviceID: JobID? = nil
    ) -> DeviceDisconnectedProperties {
        return DeviceDisconnectedProperties(
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
struct Entry: Codable {
    let properties: EntryCreatedProperties
    let entryRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case entryRequired = "required"
        case type
    }
}

// MARK: Entry convenience initializers and mutators

extension Entry {
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
        properties: EntryCreatedProperties? = nil,
        entryRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> Entry {
        return Entry(
            properties: properties ?? self.properties,
            entryRequired: entryRequired ?? self.entryRequired,
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

// MARK: - EntryCreatedProperties
struct EntryCreatedProperties: Codable {
    let entryID, libraryID: JobID

    enum CodingKeys: String, CodingKey {
        case entryID = "entry_id"
        case libraryID = "library_id"
    }
}

// MARK: EntryCreatedProperties convenience initializers and mutators

extension EntryCreatedProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(EntryCreatedProperties.self, from: data)
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
        entryID: JobID? = nil,
        libraryID: JobID? = nil
    ) -> EntryCreatedProperties {
        return EntryCreatedProperties(
            entryID: entryID ?? self.entryID,
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

// MARK: - EntryMoved
struct EntryMoved: Codable {
    let properties: EntryMovedProperties
    let entryMovedRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case entryMovedRequired = "required"
        case type
    }
}

// MARK: EntryMoved convenience initializers and mutators

extension EntryMoved {
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
        properties: EntryMovedProperties? = nil,
        entryMovedRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> EntryMoved {
        return EntryMoved(
            properties: properties ?? self.properties,
            entryMovedRequired: entryMovedRequired ?? self.entryMovedRequired,
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

// MARK: - EntryMovedProperties
struct EntryMovedProperties: Codable {
    let entryID, libraryID: JobID
    let newPath, oldPath: Destination

    enum CodingKeys: String, CodingKey {
        case entryID = "entry_id"
        case libraryID = "library_id"
        case newPath = "new_path"
        case oldPath = "old_path"
    }
}

// MARK: EntryMovedProperties convenience initializers and mutators

extension EntryMovedProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(EntryMovedProperties.self, from: data)
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
        entryID: JobID? = nil,
        libraryID: JobID? = nil,
        newPath: Destination? = nil,
        oldPath: Destination? = nil
    ) -> EntryMovedProperties {
        return EntryMovedProperties(
            entryID: entryID ?? self.entryID,
            libraryID: libraryID ?? self.libraryID,
            newPath: newPath ?? self.newPath,
            oldPath: oldPath ?? self.oldPath
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
struct FileOperationCompleted: Codable {
    let properties: FileOperationCompletedProperties
    let fileOperationCompletedRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case fileOperationCompletedRequired = "required"
        case type
    }
}

// MARK: FileOperationCompleted convenience initializers and mutators

extension FileOperationCompleted {
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
        properties: FileOperationCompletedProperties? = nil,
        fileOperationCompletedRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> FileOperationCompleted {
        return FileOperationCompleted(
            properties: properties ?? self.properties,
            fileOperationCompletedRequired: fileOperationCompletedRequired ?? self.fileOperationCompletedRequired,
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

// MARK: - FileOperationCompletedProperties
struct FileOperationCompletedProperties: Codable {
    let affectedFiles: CapacityFree
    let libraryID: JobID
    let operation: ContentDuration

    enum CodingKeys: String, CodingKey {
        case affectedFiles = "affected_files"
        case libraryID = "library_id"
        case operation
    }
}

// MARK: FileOperationCompletedProperties convenience initializers and mutators

extension FileOperationCompletedProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(FileOperationCompletedProperties.self, from: data)
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
        affectedFiles: CapacityFree? = nil,
        libraryID: JobID? = nil,
        operation: ContentDuration? = nil
    ) -> FileOperationCompletedProperties {
        return FileOperationCompletedProperties(
            affectedFiles: affectedFiles ?? self.affectedFiles,
            libraryID: libraryID ?? self.libraryID,
            operation: operation ?? self.operation
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
struct FilesIndexed: Codable {
    let properties: FilesIndexedProperties
    let filesIndexedRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case filesIndexedRequired = "required"
        case type
    }
}

// MARK: FilesIndexed convenience initializers and mutators

extension FilesIndexed {
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
        properties: FilesIndexedProperties? = nil,
        filesIndexedRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> FilesIndexed {
        return FilesIndexed(
            properties: properties ?? self.properties,
            filesIndexedRequired: filesIndexedRequired ?? self.filesIndexedRequired,
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

// MARK: - FilesIndexedProperties
struct FilesIndexedProperties: Codable {
    let count: CapacityFree
    let libraryID, locationID: JobID

    enum CodingKeys: String, CodingKey {
        case count
        case libraryID = "library_id"
        case locationID = "location_id"
    }
}

// MARK: FilesIndexedProperties convenience initializers and mutators

extension FilesIndexedProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(FilesIndexedProperties.self, from: data)
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
        count: CapacityFree? = nil,
        libraryID: JobID? = nil,
        locationID: JobID? = nil
    ) -> FilesIndexedProperties {
        return FilesIndexedProperties(
            count: count ?? self.count,
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

// MARK: - FilesModified
struct FilesModified: Codable {
    let properties: FilesModifiedProperties
    let filesModifiedRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case filesModifiedRequired = "required"
        case type
    }
}

// MARK: FilesModified convenience initializers and mutators

extension FilesModified {
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
        properties: FilesModifiedProperties? = nil,
        filesModifiedRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> FilesModified {
        return FilesModified(
            properties: properties ?? self.properties,
            filesModifiedRequired: filesModifiedRequired ?? self.filesModifiedRequired,
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

// MARK: - FilesModifiedProperties
struct FilesModifiedProperties: Codable {
    let libraryID: JobID
    let paths: PurplePaths

    enum CodingKeys: String, CodingKey {
        case libraryID = "library_id"
        case paths
    }
}

// MARK: FilesModifiedProperties convenience initializers and mutators

extension FilesModifiedProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(FilesModifiedProperties.self, from: data)
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
        libraryID: JobID? = nil,
        paths: PurplePaths? = nil
    ) -> FilesModifiedProperties {
        return FilesModifiedProperties(
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

// MARK: - PurplePaths
struct PurplePaths: Codable {
    let items: Destination
    let type: String
}

// MARK: PurplePaths convenience initializers and mutators

extension PurplePaths {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(PurplePaths.self, from: data)
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
        items: Destination? = nil,
        type: String? = nil
    ) -> PurplePaths {
        return PurplePaths(
            items: items ?? self.items,
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

// MARK: - FSRawChange
struct FSRawChange: Codable {
    let properties: FSRawChangeProperties
    let fsRawChangeRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case fsRawChangeRequired = "required"
        case type
    }
}

// MARK: FSRawChange convenience initializers and mutators

extension FSRawChange {
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
        properties: FSRawChangeProperties? = nil,
        fsRawChangeRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> FSRawChange {
        return FSRawChange(
            properties: properties ?? self.properties,
            fsRawChangeRequired: fsRawChangeRequired ?? self.fsRawChangeRequired,
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

// MARK: - FSRawChangeProperties
struct FSRawChangeProperties: Codable {
    let kind: ContentDuration
    let libraryID: JobID

    enum CodingKeys: String, CodingKey {
        case kind
        case libraryID = "library_id"
    }
}

// MARK: FSRawChangeProperties convenience initializers and mutators

extension FSRawChangeProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(FSRawChangeProperties.self, from: data)
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
        kind: ContentDuration? = nil,
        libraryID: JobID? = nil
    ) -> FSRawChangeProperties {
        return FSRawChangeProperties(
            kind: kind ?? self.kind,
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

// MARK: - IndexingCompleted
struct IndexingCompleted: Codable {
    let properties: IndexingCompletedProperties
    let indexingCompletedRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case indexingCompletedRequired = "required"
        case type
    }
}

// MARK: IndexingCompleted convenience initializers and mutators

extension IndexingCompleted {
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
        properties: IndexingCompletedProperties? = nil,
        indexingCompletedRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> IndexingCompleted {
        return IndexingCompleted(
            properties: properties ?? self.properties,
            indexingCompletedRequired: indexingCompletedRequired ?? self.indexingCompletedRequired,
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

// MARK: - IndexingCompletedProperties
struct IndexingCompletedProperties: Codable {
    let locationID: JobID
    let totalDirs, totalFiles: CapacityFree

    enum CodingKeys: String, CodingKey {
        case locationID = "location_id"
        case totalDirs = "total_dirs"
        case totalFiles = "total_files"
    }
}

// MARK: IndexingCompletedProperties convenience initializers and mutators

extension IndexingCompletedProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(IndexingCompletedProperties.self, from: data)
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
        locationID: JobID? = nil,
        totalDirs: CapacityFree? = nil,
        totalFiles: CapacityFree? = nil
    ) -> IndexingCompletedProperties {
        return IndexingCompletedProperties(
            locationID: locationID ?? self.locationID,
            totalDirs: totalDirs ?? self.totalDirs,
            totalFiles: totalFiles ?? self.totalFiles
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
struct IndexingFailed: Codable {
    let properties: IndexingFailedProperties
    let indexingFailedRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case indexingFailedRequired = "required"
        case type
    }
}

// MARK: IndexingFailed convenience initializers and mutators

extension IndexingFailed {
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
        properties: IndexingFailedProperties? = nil,
        indexingFailedRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> IndexingFailed {
        return IndexingFailed(
            properties: properties ?? self.properties,
            indexingFailedRequired: indexingFailedRequired ?? self.indexingFailedRequired,
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

// MARK: - IndexingFailedProperties
struct IndexingFailedProperties: Codable {
    let error: Destination
    let locationID: JobID

    enum CodingKeys: String, CodingKey {
        case error
        case locationID = "location_id"
    }
}

// MARK: IndexingFailedProperties convenience initializers and mutators

extension IndexingFailedProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(IndexingFailedProperties.self, from: data)
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
        error: Destination? = nil,
        locationID: JobID? = nil
    ) -> IndexingFailedProperties {
        return IndexingFailedProperties(
            error: error ?? self.error,
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

// MARK: - IndexingProgress
struct IndexingProgress: Codable {
    let properties: IndexingProgressProperties
    let indexingProgressRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case indexingProgressRequired = "required"
        case type
    }
}

// MARK: IndexingProgress convenience initializers and mutators

extension IndexingProgress {
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
        properties: IndexingProgressProperties? = nil,
        indexingProgressRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> IndexingProgress {
        return IndexingProgress(
            properties: properties ?? self.properties,
            indexingProgressRequired: indexingProgressRequired ?? self.indexingProgressRequired,
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

// MARK: - IndexingProgressProperties
struct IndexingProgressProperties: Codable {
    let locationID: JobID
    let processed: CapacityFree
    let total: AvgMemoryBytes

    enum CodingKeys: String, CodingKey {
        case locationID = "location_id"
        case processed, total
    }
}

// MARK: IndexingProgressProperties convenience initializers and mutators

extension IndexingProgressProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(IndexingProgressProperties.self, from: data)
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
        locationID: JobID? = nil,
        processed: CapacityFree? = nil,
        total: AvgMemoryBytes? = nil
    ) -> IndexingProgressProperties {
        return IndexingProgressProperties(
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
struct IndexingStarted: Codable {
    let properties: IndexingStartedProperties
    let indexingStartedRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case indexingStartedRequired = "required"
        case type
    }
}

// MARK: IndexingStarted convenience initializers and mutators

extension IndexingStarted {
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
        properties: IndexingStartedProperties? = nil,
        indexingStartedRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> IndexingStarted {
        return IndexingStarted(
            properties: properties ?? self.properties,
            indexingStartedRequired: indexingStartedRequired ?? self.indexingStartedRequired,
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

// MARK: - IndexingStartedProperties
struct IndexingStartedProperties: Codable {
    let locationID: JobID

    enum CodingKeys: String, CodingKey {
        case locationID = "location_id"
    }
}

// MARK: IndexingStartedProperties convenience initializers and mutators

extension IndexingStartedProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(IndexingStartedProperties.self, from: data)
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
        locationID: JobID? = nil
    ) -> IndexingStartedProperties {
        return IndexingStartedProperties(
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
struct JobCancelledClass: Codable {
    let properties: JobCancelledProperties
    let jobRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case jobRequired = "required"
        case type
    }
}

// MARK: JobCancelledClass convenience initializers and mutators

extension JobCancelledClass {
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
        properties: JobCancelledProperties? = nil,
        jobRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> JobCancelledClass {
        return JobCancelledClass(
            properties: properties ?? self.properties,
            jobRequired: jobRequired ?? self.jobRequired,
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

// MARK: - JobCancelledProperties
struct JobCancelledProperties: Codable {
    let jobID, jobType: Destination

    enum CodingKeys: String, CodingKey {
        case jobID = "job_id"
        case jobType = "job_type"
    }
}

// MARK: JobCancelledProperties convenience initializers and mutators

extension JobCancelledProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(JobCancelledProperties.self, from: data)
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
        jobID: Destination? = nil,
        jobType: Destination? = nil
    ) -> JobCancelledProperties {
        return JobCancelledProperties(
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
struct JobCompleted: Codable {
    let properties: JobCompletedProperties
    let jobCompletedRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case jobCompletedRequired = "required"
        case type
    }
}

// MARK: JobCompleted convenience initializers and mutators

extension JobCompleted {
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
        properties: JobCompletedProperties? = nil,
        jobCompletedRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> JobCompleted {
        return JobCompleted(
            properties: properties ?? self.properties,
            jobCompletedRequired: jobCompletedRequired ?? self.jobCompletedRequired,
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

// MARK: - JobCompletedProperties
struct JobCompletedProperties: Codable {
    let jobID, jobType: Destination
    let output: ContentDuration

    enum CodingKeys: String, CodingKey {
        case jobID = "job_id"
        case jobType = "job_type"
        case output
    }
}

// MARK: JobCompletedProperties convenience initializers and mutators

extension JobCompletedProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(JobCompletedProperties.self, from: data)
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
        jobID: Destination? = nil,
        jobType: Destination? = nil,
        output: ContentDuration? = nil
    ) -> JobCompletedProperties {
        return JobCompletedProperties(
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

// MARK: - JobFailed
struct JobFailed: Codable {
    let properties: JobFailedProperties
    let jobFailedRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case jobFailedRequired = "required"
        case type
    }
}

// MARK: JobFailed convenience initializers and mutators

extension JobFailed {
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
        properties: JobFailedProperties? = nil,
        jobFailedRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> JobFailed {
        return JobFailed(
            properties: properties ?? self.properties,
            jobFailedRequired: jobFailedRequired ?? self.jobFailedRequired,
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

// MARK: - JobFailedProperties
struct JobFailedProperties: Codable {
    let error, jobID, jobType: Destination

    enum CodingKeys: String, CodingKey {
        case error
        case jobID = "job_id"
        case jobType = "job_type"
    }
}

// MARK: JobFailedProperties convenience initializers and mutators

extension JobFailedProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(JobFailedProperties.self, from: data)
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
        error: Destination? = nil,
        jobID: Destination? = nil,
        jobType: Destination? = nil
    ) -> JobFailedProperties {
        return JobFailedProperties(
            error: error ?? self.error,
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

// MARK: - JobPausedClass
struct JobPausedClass: Codable {
    let properties: JobPausedProperties
    let jobRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case jobRequired = "required"
        case type
    }
}

// MARK: JobPausedClass convenience initializers and mutators

extension JobPausedClass {
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
        properties: JobPausedProperties? = nil,
        jobRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> JobPausedClass {
        return JobPausedClass(
            properties: properties ?? self.properties,
            jobRequired: jobRequired ?? self.jobRequired,
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

// MARK: - JobPausedProperties
struct JobPausedProperties: Codable {
    let jobID: Destination

    enum CodingKeys: String, CodingKey {
        case jobID = "job_id"
    }
}

// MARK: JobPausedProperties convenience initializers and mutators

extension JobPausedProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(JobPausedProperties.self, from: data)
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
        jobID: Destination? = nil
    ) -> JobPausedProperties {
        return JobPausedProperties(
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
struct JobProgress: Codable {
    let properties: JobProgressProperties
    let jobProgressRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case jobProgressRequired = "required"
        case type
    }
}

// MARK: JobProgress convenience initializers and mutators

extension JobProgress {
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
        properties: JobProgressProperties? = nil,
        jobProgressRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> JobProgress {
        return JobProgress(
            properties: properties ?? self.properties,
            jobProgressRequired: jobProgressRequired ?? self.jobProgressRequired,
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

// MARK: - JobProgressProperties
struct JobProgressProperties: Codable {
    let genericProgress: Bool
    let jobID, jobType: Destination
    let message: ErrorMessage
    let progress: JobID

    enum CodingKeys: String, CodingKey {
        case genericProgress = "generic_progress"
        case jobID = "job_id"
        case jobType = "job_type"
        case message, progress
    }
}

// MARK: JobProgressProperties convenience initializers and mutators

extension JobProgressProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(JobProgressProperties.self, from: data)
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
        genericProgress: Bool? = nil,
        jobID: Destination? = nil,
        jobType: Destination? = nil,
        message: ErrorMessage? = nil,
        progress: JobID? = nil
    ) -> JobProgressProperties {
        return JobProgressProperties(
            genericProgress: genericProgress ?? self.genericProgress,
            jobID: jobID ?? self.jobID,
            jobType: jobType ?? self.jobType,
            message: message ?? self.message,
            progress: progress ?? self.progress
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
struct LibraryClosed: Codable {
    let properties: LibraryClosedProperties
    let libraryClosedRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case libraryClosedRequired = "required"
        case type
    }
}

// MARK: LibraryClosed convenience initializers and mutators

extension LibraryClosed {
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
        properties: LibraryClosedProperties? = nil,
        libraryClosedRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> LibraryClosed {
        return LibraryClosed(
            properties: properties ?? self.properties,
            libraryClosedRequired: libraryClosedRequired ?? self.libraryClosedRequired,
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

// MARK: - LibraryClosedProperties
struct LibraryClosedProperties: Codable {
    let id: JobID
    let name: Destination
}

// MARK: LibraryClosedProperties convenience initializers and mutators

extension LibraryClosedProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(LibraryClosedProperties.self, from: data)
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
        id: JobID? = nil,
        name: Destination? = nil
    ) -> LibraryClosedProperties {
        return LibraryClosedProperties(
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
struct Library: Codable {
    let properties: LibraryCreatedProperties
    let libraryRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case libraryRequired = "required"
        case type
    }
}

// MARK: Library convenience initializers and mutators

extension Library {
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
        properties: LibraryCreatedProperties? = nil,
        libraryRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> Library {
        return Library(
            properties: properties ?? self.properties,
            libraryRequired: libraryRequired ?? self.libraryRequired,
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

// MARK: - LibraryCreatedProperties
struct LibraryCreatedProperties: Codable {
    let id: JobID
    let name, path: Destination
}

// MARK: LibraryCreatedProperties convenience initializers and mutators

extension LibraryCreatedProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(LibraryCreatedProperties.self, from: data)
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
        id: JobID? = nil,
        name: Destination? = nil,
        path: Destination? = nil
    ) -> LibraryCreatedProperties {
        return LibraryCreatedProperties(
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
struct LibraryDeleted: Codable {
    let properties: LibraryDeletedProperties
    let libraryDeletedRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case libraryDeletedRequired = "required"
        case type
    }
}

// MARK: LibraryDeleted convenience initializers and mutators

extension LibraryDeleted {
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
        properties: LibraryDeletedProperties? = nil,
        libraryDeletedRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> LibraryDeleted {
        return LibraryDeleted(
            properties: properties ?? self.properties,
            libraryDeletedRequired: libraryDeletedRequired ?? self.libraryDeletedRequired,
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

// MARK: - LibraryDeletedProperties
struct LibraryDeletedProperties: Codable {
    let deletedData: Destination
    let id: JobID
    let name: Destination

    enum CodingKeys: String, CodingKey {
        case deletedData = "deleted_data"
        case id, name
    }
}

// MARK: LibraryDeletedProperties convenience initializers and mutators

extension LibraryDeletedProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(LibraryDeletedProperties.self, from: data)
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
        deletedData: Destination? = nil,
        id: JobID? = nil,
        name: Destination? = nil
    ) -> LibraryDeletedProperties {
        return LibraryDeletedProperties(
            deletedData: deletedData ?? self.deletedData,
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

// MARK: - LocationAdded
struct LocationAdded: Codable {
    let properties: LocationAddedProperties
    let locationAddedRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case locationAddedRequired = "required"
        case type
    }
}

// MARK: LocationAdded convenience initializers and mutators

extension LocationAdded {
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
        properties: LocationAddedProperties? = nil,
        locationAddedRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> LocationAdded {
        return LocationAdded(
            properties: properties ?? self.properties,
            locationAddedRequired: locationAddedRequired ?? self.locationAddedRequired,
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

// MARK: - LocationAddedProperties
struct LocationAddedProperties: Codable {
    let libraryID, locationID: JobID
    let path: Destination

    enum CodingKeys: String, CodingKey {
        case libraryID = "library_id"
        case locationID = "location_id"
        case path
    }
}

// MARK: LocationAddedProperties convenience initializers and mutators

extension LocationAddedProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(LocationAddedProperties.self, from: data)
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
        libraryID: JobID? = nil,
        locationID: JobID? = nil,
        path: Destination? = nil
    ) -> LocationAddedProperties {
        return LocationAddedProperties(
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
struct LocationRemoved: Codable {
    let properties: LocationRemovedProperties
    let locationRemovedRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case locationRemovedRequired = "required"
        case type
    }
}

// MARK: LocationRemoved convenience initializers and mutators

extension LocationRemoved {
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
        properties: LocationRemovedProperties? = nil,
        locationRemovedRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> LocationRemoved {
        return LocationRemoved(
            properties: properties ?? self.properties,
            locationRemovedRequired: locationRemovedRequired ?? self.locationRemovedRequired,
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

// MARK: - LocationRemovedProperties
struct LocationRemovedProperties: Codable {
    let libraryID, locationID: JobID

    enum CodingKeys: String, CodingKey {
        case libraryID = "library_id"
        case locationID = "location_id"
    }
}

// MARK: LocationRemovedProperties convenience initializers and mutators

extension LocationRemovedProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(LocationRemovedProperties.self, from: data)
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
        libraryID: JobID? = nil,
        locationID: JobID? = nil
    ) -> LocationRemovedProperties {
        return LocationRemovedProperties(
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
struct LogMessage: Codable {
    let properties: LogMessageProperties
    let logMessageRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case logMessageRequired = "required"
        case type
    }
}

// MARK: LogMessage convenience initializers and mutators

extension LogMessage {
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
        properties: LogMessageProperties? = nil,
        logMessageRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> LogMessage {
        return LogMessage(
            properties: properties ?? self.properties,
            logMessageRequired: logMessageRequired ?? self.logMessageRequired,
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

// MARK: - LogMessageProperties
struct LogMessageProperties: Codable {
    let jobID: ErrorMessage
    let level: Destination
    let libraryID: CompletedAt
    let message, target: Destination
    let timestamp: JobID

    enum CodingKeys: String, CodingKey {
        case jobID = "job_id"
        case level
        case libraryID = "library_id"
        case message, target, timestamp
    }
}

// MARK: LogMessageProperties convenience initializers and mutators

extension LogMessageProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(LogMessageProperties.self, from: data)
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
        jobID: ErrorMessage? = nil,
        level: Destination? = nil,
        libraryID: CompletedAt? = nil,
        message: Destination? = nil,
        target: Destination? = nil,
        timestamp: JobID? = nil
    ) -> LogMessageProperties {
        return LogMessageProperties(
            jobID: jobID ?? self.jobID,
            level: level ?? self.level,
            libraryID: libraryID ?? self.libraryID,
            message: message ?? self.message,
            target: target ?? self.target,
            timestamp: timestamp ?? self.timestamp
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - CompletedAt
struct CompletedAt: Codable {
    let format: Format
    let type: [ContainerVolumeIDType]
}

// MARK: CompletedAt convenience initializers and mutators

extension CompletedAt {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(CompletedAt.self, from: data)
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
        format: Format? = nil,
        type: [ContainerVolumeIDType]? = nil
    ) -> CompletedAt {
        return CompletedAt(
            format: format ?? self.format,
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

// MARK: - ThumbnailsGenerated
struct ThumbnailsGenerated: Codable {
    let properties: ThumbnailsGeneratedProperties
    let thumbnailsGeneratedRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case thumbnailsGeneratedRequired = "required"
        case type
    }
}

// MARK: ThumbnailsGenerated convenience initializers and mutators

extension ThumbnailsGenerated {
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
        properties: ThumbnailsGeneratedProperties? = nil,
        thumbnailsGeneratedRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> ThumbnailsGenerated {
        return ThumbnailsGenerated(
            properties: properties ?? self.properties,
            thumbnailsGeneratedRequired: thumbnailsGeneratedRequired ?? self.thumbnailsGeneratedRequired,
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

// MARK: - ThumbnailsGeneratedProperties
struct ThumbnailsGeneratedProperties: Codable {
    let count: CapacityFree
    let libraryID: JobID

    enum CodingKeys: String, CodingKey {
        case count
        case libraryID = "library_id"
    }
}

// MARK: ThumbnailsGeneratedProperties convenience initializers and mutators

extension ThumbnailsGeneratedProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(ThumbnailsGeneratedProperties.self, from: data)
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
        count: CapacityFree? = nil,
        libraryID: JobID? = nil
    ) -> ThumbnailsGeneratedProperties {
        return ThumbnailsGeneratedProperties(
            count: count ?? self.count,
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

// MARK: - VolumeError
struct VolumeError: Codable {
    let properties: VolumeErrorProperties
    let volumeErrorRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case volumeErrorRequired = "required"
        case type
    }
}

// MARK: VolumeError convenience initializers and mutators

extension VolumeError {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(VolumeError.self, from: data)
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
        properties: VolumeErrorProperties? = nil,
        volumeErrorRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> VolumeError {
        return VolumeError(
            properties: properties ?? self.properties,
            volumeErrorRequired: volumeErrorRequired ?? self.volumeErrorRequired,
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

// MARK: - VolumeErrorProperties
struct VolumeErrorProperties: Codable {
    let error: Destination
    let fingerprint: ContentDuration
}

// MARK: VolumeErrorProperties convenience initializers and mutators

extension VolumeErrorProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(VolumeErrorProperties.self, from: data)
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
        error: Destination? = nil,
        fingerprint: ContentDuration? = nil
    ) -> VolumeErrorProperties {
        return VolumeErrorProperties(
            error: error ?? self.error,
            fingerprint: fingerprint ?? self.fingerprint
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - VolumeMountChanged
struct VolumeMountChanged: Codable {
    let properties: VolumeMountChangedProperties
    let volumeMountChangedRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case volumeMountChangedRequired = "required"
        case type
    }
}

// MARK: VolumeMountChanged convenience initializers and mutators

extension VolumeMountChanged {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(VolumeMountChanged.self, from: data)
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
        properties: VolumeMountChangedProperties? = nil,
        volumeMountChangedRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> VolumeMountChanged {
        return VolumeMountChanged(
            properties: properties ?? self.properties,
            volumeMountChangedRequired: volumeMountChangedRequired ?? self.volumeMountChangedRequired,
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

// MARK: - VolumeMountChangedProperties
struct VolumeMountChangedProperties: Codable {
    let fingerprint: ContentDuration
    let isMounted: Destination

    enum CodingKeys: String, CodingKey {
        case fingerprint
        case isMounted = "is_mounted"
    }
}

// MARK: VolumeMountChangedProperties convenience initializers and mutators

extension VolumeMountChangedProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(VolumeMountChangedProperties.self, from: data)
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
        fingerprint: ContentDuration? = nil,
        isMounted: Destination? = nil
    ) -> VolumeMountChangedProperties {
        return VolumeMountChangedProperties(
            fingerprint: fingerprint ?? self.fingerprint,
            isMounted: isMounted ?? self.isMounted
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - VolumeRemoved
struct VolumeRemoved: Codable {
    let properties: VolumeRemovedProperties
    let volumeRemovedRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case volumeRemovedRequired = "required"
        case type
    }
}

// MARK: VolumeRemoved convenience initializers and mutators

extension VolumeRemoved {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(VolumeRemoved.self, from: data)
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
        properties: VolumeRemovedProperties? = nil,
        volumeRemovedRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> VolumeRemoved {
        return VolumeRemoved(
            properties: properties ?? self.properties,
            volumeRemovedRequired: volumeRemovedRequired ?? self.volumeRemovedRequired,
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

// MARK: - VolumeRemovedProperties
struct VolumeRemovedProperties: Codable {
    let fingerprint: ContentDuration
}

// MARK: VolumeRemovedProperties convenience initializers and mutators

extension VolumeRemovedProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(VolumeRemovedProperties.self, from: data)
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
        fingerprint: ContentDuration? = nil
    ) -> VolumeRemovedProperties {
        return VolumeRemovedProperties(
            fingerprint: fingerprint ?? self.fingerprint
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - VolumeSpeedTested
struct VolumeSpeedTested: Codable {
    let properties: VolumeSpeedTestedProperties
    let volumeSpeedTestedRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case volumeSpeedTestedRequired = "required"
        case type
    }
}

// MARK: VolumeSpeedTested convenience initializers and mutators

extension VolumeSpeedTested {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(VolumeSpeedTested.self, from: data)
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
        properties: VolumeSpeedTestedProperties? = nil,
        volumeSpeedTestedRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> VolumeSpeedTested {
        return VolumeSpeedTested(
            properties: properties ?? self.properties,
            volumeSpeedTestedRequired: volumeSpeedTestedRequired ?? self.volumeSpeedTestedRequired,
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

// MARK: - VolumeSpeedTestedProperties
struct VolumeSpeedTestedProperties: Codable {
    let fingerprint: ContentDuration
    let readSpeedMbps, writeSpeedMbps: CapacityFree

    enum CodingKeys: String, CodingKey {
        case fingerprint
        case readSpeedMbps = "read_speed_mbps"
        case writeSpeedMbps = "write_speed_mbps"
    }
}

// MARK: VolumeSpeedTestedProperties convenience initializers and mutators

extension VolumeSpeedTestedProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(VolumeSpeedTestedProperties.self, from: data)
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
        fingerprint: ContentDuration? = nil,
        readSpeedMbps: CapacityFree? = nil,
        writeSpeedMbps: CapacityFree? = nil
    ) -> VolumeSpeedTestedProperties {
        return VolumeSpeedTestedProperties(
            fingerprint: fingerprint ?? self.fingerprint,
            readSpeedMbps: readSpeedMbps ?? self.readSpeedMbps,
            writeSpeedMbps: writeSpeedMbps ?? self.writeSpeedMbps
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - VolumeUpdated
struct VolumeUpdated: Codable {
    let properties: VolumeUpdatedProperties
    let volumeUpdatedRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case volumeUpdatedRequired = "required"
        case type
    }
}

// MARK: VolumeUpdated convenience initializers and mutators

extension VolumeUpdated {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(VolumeUpdated.self, from: data)
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
        properties: VolumeUpdatedProperties? = nil,
        volumeUpdatedRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> VolumeUpdated {
        return VolumeUpdated(
            properties: properties ?? self.properties,
            volumeUpdatedRequired: volumeUpdatedRequired ?? self.volumeUpdatedRequired,
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

// MARK: - VolumeUpdatedProperties
struct VolumeUpdatedProperties: Codable {
    let fingerprint, newInfo, oldInfo: ContentDuration

    enum CodingKeys: String, CodingKey {
        case fingerprint
        case newInfo = "new_info"
        case oldInfo = "old_info"
    }
}

// MARK: VolumeUpdatedProperties convenience initializers and mutators

extension VolumeUpdatedProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(VolumeUpdatedProperties.self, from: data)
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
        fingerprint: ContentDuration? = nil,
        newInfo: ContentDuration? = nil,
        oldInfo: ContentDuration? = nil
    ) -> VolumeUpdatedProperties {
        return VolumeUpdatedProperties(
            fingerprint: fingerprint ?? self.fingerprint,
            newInfo: newInfo ?? self.newInfo,
            oldInfo: oldInfo ?? self.oldInfo
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - FileCopyActionOutput
struct FileCopyActionOutput: Codable {
    let schema: String
    let description: String
    let properties: FileCopyActionOutputProperties
    let fileCopyActionOutputRequired: [String]
    let title: String
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case schema = "$schema"
        case description, properties
        case fileCopyActionOutputRequired = "required"
        case title, type
    }
}

// MARK: FileCopyActionOutput convenience initializers and mutators

extension FileCopyActionOutput {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(FileCopyActionOutput.self, from: data)
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
        schema: String? = nil,
        description: String? = nil,
        properties: FileCopyActionOutputProperties? = nil,
        fileCopyActionOutputRequired: [String]? = nil,
        title: String? = nil,
        type: OutputType? = nil
    ) -> FileCopyActionOutput {
        return FileCopyActionOutput(
            schema: schema ?? self.schema,
            description: description ?? self.description,
            properties: properties ?? self.properties,
            fileCopyActionOutputRequired: fileCopyActionOutputRequired ?? self.fileCopyActionOutputRequired,
            title: title ?? self.title,
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

// MARK: - FileCopyActionOutputProperties
struct FileCopyActionOutputProperties: Codable {
    let destination: Destination
    let jobID: JobID
    let sourcesCount: CapacityFree

    enum CodingKeys: String, CodingKey {
        case destination
        case jobID = "job_id"
        case sourcesCount = "sources_count"
    }
}

// MARK: FileCopyActionOutputProperties convenience initializers and mutators

extension FileCopyActionOutputProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(FileCopyActionOutputProperties.self, from: data)
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
        destination: Destination? = nil,
        jobID: JobID? = nil,
        sourcesCount: CapacityFree? = nil
    ) -> FileCopyActionOutputProperties {
        return FileCopyActionOutputProperties(
            destination: destination ?? self.destination,
            jobID: jobID ?? self.jobID,
            sourcesCount: sourcesCount ?? self.sourcesCount
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - FileOperation
struct FileOperation: Codable {
    let schema: String
    let description: String
    let fileOperationEnum: [String]?
    let title: String
    let type: DestinationType

    enum CodingKeys: String, CodingKey {
        case schema = "$schema"
        case description
        case fileOperationEnum = "enum"
        case title, type
    }
}

// MARK: FileOperation convenience initializers and mutators

extension FileOperation {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(FileOperation.self, from: data)
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
        schema: String? = nil,
        description: String? = nil,
        fileOperationEnum: [String]?? = nil,
        title: String? = nil,
        type: DestinationType? = nil
    ) -> FileOperation {
        return FileOperation(
            schema: schema ?? self.schema,
            description: description ?? self.description,
            fileOperationEnum: fileOperationEnum ?? self.fileOperationEnum,
            title: title ?? self.title,
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

// MARK: - TypesFSRawEventKind
struct TypesFSRawEventKind: Codable {
    let schema: String
    let description: String
    let oneOf: [FSRawEventKindOneOf]
    let title: String

    enum CodingKeys: String, CodingKey {
        case schema = "$schema"
        case description, oneOf, title
    }
}

// MARK: TypesFSRawEventKind convenience initializers and mutators

extension TypesFSRawEventKind {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(TypesFSRawEventKind.self, from: data)
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
        schema: String? = nil,
        description: String? = nil,
        oneOf: [FSRawEventKindOneOf]? = nil,
        title: String? = nil
    ) -> TypesFSRawEventKind {
        return TypesFSRawEventKind(
            schema: schema ?? self.schema,
            description: description ?? self.description,
            oneOf: oneOf ?? self.oneOf,
            title: title ?? self.title
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - TypesGenericProgress
struct TypesGenericProgress: Codable {
    let schema: String
    let definitions: GenericProgressDefinitions
    let description: String
    let properties: GenericProgressProperties
    let genericProgressRequired: [String]
    let title: String
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case schema = "$schema"
        case definitions, description, properties
        case genericProgressRequired = "required"
        case title, type
    }
}

// MARK: TypesGenericProgress convenience initializers and mutators

extension TypesGenericProgress {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(TypesGenericProgress.self, from: data)
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
        schema: String? = nil,
        definitions: GenericProgressDefinitions? = nil,
        description: String? = nil,
        properties: GenericProgressProperties? = nil,
        genericProgressRequired: [String]? = nil,
        title: String? = nil,
        type: OutputType? = nil
    ) -> TypesGenericProgress {
        return TypesGenericProgress(
            schema: schema ?? self.schema,
            definitions: definitions ?? self.definitions,
            description: description ?? self.description,
            properties: properties ?? self.properties,
            genericProgressRequired: genericProgressRequired ?? self.genericProgressRequired,
            title: title ?? self.title,
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

// MARK: - GenericProgressDefinitions
struct GenericProgressDefinitions: Codable {
    let duration: Duration
    let performanceMetrics: DefinitionsPerformanceMetrics
    let progressCompletion: DefinitionsProgressCompletion
    let sdPath: DefinitionsSDPath
    let genericProgress: DefinitionsGenericProgress?

    enum CodingKeys: String, CodingKey {
        case duration = "Duration"
        case performanceMetrics = "PerformanceMetrics"
        case progressCompletion = "ProgressCompletion"
        case sdPath = "SdPath"
        case genericProgress = "GenericProgress"
    }
}

// MARK: GenericProgressDefinitions convenience initializers and mutators

extension GenericProgressDefinitions {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(GenericProgressDefinitions.self, from: data)
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
        duration: Duration? = nil,
        performanceMetrics: DefinitionsPerformanceMetrics? = nil,
        progressCompletion: DefinitionsProgressCompletion? = nil,
        sdPath: DefinitionsSDPath? = nil,
        genericProgress: DefinitionsGenericProgress?? = nil
    ) -> GenericProgressDefinitions {
        return GenericProgressDefinitions(
            duration: duration ?? self.duration,
            performanceMetrics: performanceMetrics ?? self.performanceMetrics,
            progressCompletion: progressCompletion ?? self.progressCompletion,
            sdPath: sdPath ?? self.sdPath,
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

// MARK: - DefinitionsGenericProgress
struct DefinitionsGenericProgress: Codable {
    let description: String
    let properties: GenericProgressProperties
    let genericProgressRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case description, properties
        case genericProgressRequired = "required"
        case type
    }
}

// MARK: DefinitionsGenericProgress convenience initializers and mutators

extension DefinitionsGenericProgress {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(DefinitionsGenericProgress.self, from: data)
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
        description: String? = nil,
        properties: GenericProgressProperties? = nil,
        genericProgressRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> DefinitionsGenericProgress {
        return DefinitionsGenericProgress(
            description: description ?? self.description,
            properties: properties ?? self.properties,
            genericProgressRequired: genericProgressRequired ?? self.genericProgressRequired,
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

// MARK: - GenericProgressProperties
struct GenericProgressProperties: Codable {
    let completion: Role
    let currentPath: CurrentPath
    let message: ContainerID
    let metadata: Metadata
    let percentage: CapacityFree
    let performance: Role
    let phase: ContainerID

    enum CodingKeys: String, CodingKey {
        case completion
        case currentPath = "current_path"
        case message, metadata, percentage, performance, phase
    }
}

// MARK: GenericProgressProperties convenience initializers and mutators

extension GenericProgressProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(GenericProgressProperties.self, from: data)
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
        completion: Role? = nil,
        currentPath: CurrentPath? = nil,
        message: ContainerID? = nil,
        metadata: Metadata? = nil,
        percentage: CapacityFree? = nil,
        performance: Role? = nil,
        phase: ContainerID? = nil
    ) -> GenericProgressProperties {
        return GenericProgressProperties(
            completion: completion ?? self.completion,
            currentPath: currentPath ?? self.currentPath,
            message: message ?? self.message,
            metadata: metadata ?? self.metadata,
            percentage: percentage ?? self.percentage,
            performance: performance ?? self.performance,
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

// MARK: - Metadata
struct Metadata: Codable {
    let description: String
}

// MARK: Metadata convenience initializers and mutators

extension Metadata {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(Metadata.self, from: data)
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
        description: String? = nil
    ) -> Metadata {
        return Metadata(
            description: description ?? self.description
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - DefinitionsPerformanceMetrics
struct DefinitionsPerformanceMetrics: Codable {
    let description: String
    let properties: PerformanceMetricsProperties
    let performanceMetricsRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case description, properties
        case performanceMetricsRequired = "required"
        case type
    }
}

// MARK: DefinitionsPerformanceMetrics convenience initializers and mutators

extension DefinitionsPerformanceMetrics {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(DefinitionsPerformanceMetrics.self, from: data)
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
        description: String? = nil,
        properties: PerformanceMetricsProperties? = nil,
        performanceMetricsRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> DefinitionsPerformanceMetrics {
        return DefinitionsPerformanceMetrics(
            description: description ?? self.description,
            properties: properties ?? self.properties,
            performanceMetricsRequired: performanceMetricsRequired ?? self.performanceMetricsRequired,
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

// MARK: - PerformanceMetricsProperties
struct PerformanceMetricsProperties: Codable {
    let elapsed: CurrentPath
    let errorCount: CapacityFree
    let estimatedRemaining: CurrentPath
    let rate, warningCount: CapacityFree

    enum CodingKeys: String, CodingKey {
        case elapsed
        case errorCount = "error_count"
        case estimatedRemaining = "estimated_remaining"
        case rate
        case warningCount = "warning_count"
    }
}

// MARK: PerformanceMetricsProperties convenience initializers and mutators

extension PerformanceMetricsProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(PerformanceMetricsProperties.self, from: data)
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
        elapsed: CurrentPath? = nil,
        errorCount: CapacityFree? = nil,
        estimatedRemaining: CurrentPath? = nil,
        rate: CapacityFree? = nil,
        warningCount: CapacityFree? = nil
    ) -> PerformanceMetricsProperties {
        return PerformanceMetricsProperties(
            elapsed: elapsed ?? self.elapsed,
            errorCount: errorCount ?? self.errorCount,
            estimatedRemaining: estimatedRemaining ?? self.estimatedRemaining,
            rate: rate ?? self.rate,
            warningCount: warningCount ?? self.warningCount
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - DefinitionsProgressCompletion
struct DefinitionsProgressCompletion: Codable {
    let description: String
    let properties: ProgressCompletionProperties
    let progressCompletionRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case description, properties
        case progressCompletionRequired = "required"
        case type
    }
}

// MARK: DefinitionsProgressCompletion convenience initializers and mutators

extension DefinitionsProgressCompletion {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(DefinitionsProgressCompletion.self, from: data)
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
        description: String? = nil,
        properties: ProgressCompletionProperties? = nil,
        progressCompletionRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> DefinitionsProgressCompletion {
        return DefinitionsProgressCompletion(
            description: description ?? self.description,
            properties: properties ?? self.properties,
            progressCompletionRequired: progressCompletionRequired ?? self.progressCompletionRequired,
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

// MARK: - ProgressCompletionProperties
struct ProgressCompletionProperties: Codable {
    let bytesCompleted: AvgMemoryBytes
    let completed, total: CapacityFree
    let totalBytes: AvgMemoryBytes

    enum CodingKeys: String, CodingKey {
        case bytesCompleted = "bytes_completed"
        case completed, total
        case totalBytes = "total_bytes"
    }
}

// MARK: ProgressCompletionProperties convenience initializers and mutators

extension ProgressCompletionProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(ProgressCompletionProperties.self, from: data)
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
        bytesCompleted: AvgMemoryBytes? = nil,
        completed: CapacityFree? = nil,
        total: CapacityFree? = nil,
        totalBytes: AvgMemoryBytes? = nil
    ) -> ProgressCompletionProperties {
        return ProgressCompletionProperties(
            bytesCompleted: bytesCompleted ?? self.bytesCompleted,
            completed: completed ?? self.completed,
            total: total ?? self.total,
            totalBytes: totalBytes ?? self.totalBytes
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - DefinitionsSDPath
struct DefinitionsSDPath: Codable {
    let description: String
    let oneOf: [SDPathOneOf]
}

// MARK: DefinitionsSDPath convenience initializers and mutators

extension DefinitionsSDPath {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(DefinitionsSDPath.self, from: data)
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
        description: String? = nil,
        oneOf: [SDPathOneOf]? = nil
    ) -> DefinitionsSDPath {
        return DefinitionsSDPath(
            description: description ?? self.description,
            oneOf: oneOf ?? self.oneOf
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - SDPathOneOf
struct SDPathOneOf: Codable {
    let additionalProperties: Bool
    let description: String
    let properties: IndecentProperties
    let oneOfRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case additionalProperties, description, properties
        case oneOfRequired = "required"
        case type
    }
}

// MARK: SDPathOneOf convenience initializers and mutators

extension SDPathOneOf {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(SDPathOneOf.self, from: data)
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
        additionalProperties: Bool? = nil,
        description: String? = nil,
        properties: IndecentProperties? = nil,
        oneOfRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> SDPathOneOf {
        return SDPathOneOf(
            additionalProperties: additionalProperties ?? self.additionalProperties,
            description: description ?? self.description,
            properties: properties ?? self.properties,
            oneOfRequired: oneOfRequired ?? self.oneOfRequired,
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

// MARK: - IndecentProperties
struct IndecentProperties: Codable {
    let physical: Physical?
    let content: Content?

    enum CodingKeys: String, CodingKey {
        case physical = "Physical"
        case content = "Content"
    }
}

// MARK: IndecentProperties convenience initializers and mutators

extension IndecentProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(IndecentProperties.self, from: data)
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
        physical: Physical?? = nil,
        content: Content?? = nil
    ) -> IndecentProperties {
        return IndecentProperties(
            physical: physical ?? self.physical,
            content: content ?? self.content
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - Content
struct Content: Codable {
    let properties: ContentProperties
    let contentRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case contentRequired = "required"
        case type
    }
}

// MARK: Content convenience initializers and mutators

extension Content {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(Content.self, from: data)
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
        properties: ContentProperties? = nil,
        contentRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> Content {
        return Content(
            properties: properties ?? self.properties,
            contentRequired: contentRequired ?? self.contentRequired,
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

// MARK: - ContentProperties
struct ContentProperties: Codable {
    let contentID: CapacityFree

    enum CodingKeys: String, CodingKey {
        case contentID = "content_id"
    }
}

// MARK: ContentProperties convenience initializers and mutators

extension ContentProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(ContentProperties.self, from: data)
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
        contentID: CapacityFree? = nil
    ) -> ContentProperties {
        return ContentProperties(
            contentID: contentID ?? self.contentID
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - Physical
struct Physical: Codable {
    let properties: PhysicalProperties
    let physicalRequired: [String]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case properties
        case physicalRequired = "required"
        case type
    }
}

// MARK: Physical convenience initializers and mutators

extension Physical {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(Physical.self, from: data)
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
        properties: PhysicalProperties? = nil,
        physicalRequired: [String]? = nil,
        type: OutputType? = nil
    ) -> Physical {
        return Physical(
            properties: properties ?? self.properties,
            physicalRequired: physicalRequired ?? self.physicalRequired,
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

// MARK: - PhysicalProperties
struct PhysicalProperties: Codable {
    let deviceID: CapacityFree
    let path: ContainerID

    enum CodingKeys: String, CodingKey {
        case deviceID = "device_id"
        case path
    }
}

// MARK: PhysicalProperties convenience initializers and mutators

extension PhysicalProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(PhysicalProperties.self, from: data)
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
        deviceID: CapacityFree? = nil,
        path: ContainerID? = nil
    ) -> PhysicalProperties {
        return PhysicalProperties(
            deviceID: deviceID ?? self.deviceID,
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

// MARK: - TypesIndexerMetrics
struct TypesIndexerMetrics: Codable {
    let schema: String
    let definitions: IndexerMetricsDefinitions
    let description: String
    let properties: IndexerMetricsProperties
    let indexerMetricsRequired: [String]
    let title: String
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case schema = "$schema"
        case definitions, description, properties
        case indexerMetricsRequired = "required"
        case title, type
    }
}

// MARK: TypesIndexerMetrics convenience initializers and mutators

extension TypesIndexerMetrics {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(TypesIndexerMetrics.self, from: data)
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
        schema: String? = nil,
        definitions: IndexerMetricsDefinitions? = nil,
        description: String? = nil,
        properties: IndexerMetricsProperties? = nil,
        indexerMetricsRequired: [String]? = nil,
        title: String? = nil,
        type: OutputType? = nil
    ) -> TypesIndexerMetrics {
        return TypesIndexerMetrics(
            schema: schema ?? self.schema,
            definitions: definitions ?? self.definitions,
            description: description ?? self.description,
            properties: properties ?? self.properties,
            indexerMetricsRequired: indexerMetricsRequired ?? self.indexerMetricsRequired,
            title: title ?? self.title,
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

// MARK: - IndexerMetricsDefinitions
struct IndexerMetricsDefinitions: Codable {
    let duration: Duration

    enum CodingKeys: String, CodingKey {
        case duration = "Duration"
    }
}

// MARK: IndexerMetricsDefinitions convenience initializers and mutators

extension IndexerMetricsDefinitions {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(IndexerMetricsDefinitions.self, from: data)
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
        duration: Duration? = nil
    ) -> IndexerMetricsDefinitions {
        return IndexerMetricsDefinitions(
            duration: duration ?? self.duration
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - TypesIndexerStats
struct TypesIndexerStats: Codable {
    let schema: String
    let description: String
    let properties: IndexerStatsProperties
    let indexerStatsRequired: [String]
    let title: String
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case schema = "$schema"
        case description, properties
        case indexerStatsRequired = "required"
        case title, type
    }
}

// MARK: TypesIndexerStats convenience initializers and mutators

extension TypesIndexerStats {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(TypesIndexerStats.self, from: data)
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
        schema: String? = nil,
        description: String? = nil,
        properties: IndexerStatsProperties? = nil,
        indexerStatsRequired: [String]? = nil,
        title: String? = nil,
        type: OutputType? = nil
    ) -> TypesIndexerStats {
        return TypesIndexerStats(
            schema: schema ?? self.schema,
            description: description ?? self.description,
            properties: properties ?? self.properties,
            indexerStatsRequired: indexerStatsRequired ?? self.indexerStatsRequired,
            title: title ?? self.title,
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

// MARK: - JobInfoOutput
struct JobInfoOutput: Codable {
    let schema: String
    let definitions: JobInfoOutputDefinitions
    let properties: JobInfoOutputProperties
    let jobInfoOutputRequired: [String]
    let title: String
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case schema = "$schema"
        case definitions, properties
        case jobInfoOutputRequired = "required"
        case title, type
    }
}

// MARK: JobInfoOutput convenience initializers and mutators

extension JobInfoOutput {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(JobInfoOutput.self, from: data)
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
        schema: String? = nil,
        definitions: JobInfoOutputDefinitions? = nil,
        properties: JobInfoOutputProperties? = nil,
        jobInfoOutputRequired: [String]? = nil,
        title: String? = nil,
        type: OutputType? = nil
    ) -> JobInfoOutput {
        return JobInfoOutput(
            schema: schema ?? self.schema,
            definitions: definitions ?? self.definitions,
            properties: properties ?? self.properties,
            jobInfoOutputRequired: jobInfoOutputRequired ?? self.jobInfoOutputRequired,
            title: title ?? self.title,
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

// MARK: - JobInfoOutputDefinitions
struct JobInfoOutputDefinitions: Codable {
    let jobStatus: DefinitionsDiskType

    enum CodingKeys: String, CodingKey {
        case jobStatus = "JobStatus"
    }
}

// MARK: JobInfoOutputDefinitions convenience initializers and mutators

extension JobInfoOutputDefinitions {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(JobInfoOutputDefinitions.self, from: data)
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
        jobStatus: DefinitionsDiskType? = nil
    ) -> JobInfoOutputDefinitions {
        return JobInfoOutputDefinitions(
            jobStatus: jobStatus ?? self.jobStatus
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - JobInfoOutputProperties
struct JobInfoOutputProperties: Codable {
    let completedAt: CompletedAt
    let errorMessage: ErrorMessage
    let id: JobID
    let name: Destination
    let progress, startedAt: JobID
    let status: ContentDuration

    enum CodingKeys: String, CodingKey {
        case completedAt = "completed_at"
        case errorMessage = "error_message"
        case id, name, progress
        case startedAt = "started_at"
        case status
    }
}

// MARK: JobInfoOutputProperties convenience initializers and mutators

extension JobInfoOutputProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(JobInfoOutputProperties.self, from: data)
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
        completedAt: CompletedAt? = nil,
        errorMessage: ErrorMessage? = nil,
        id: JobID? = nil,
        name: Destination? = nil,
        progress: JobID? = nil,
        startedAt: JobID? = nil,
        status: ContentDuration? = nil
    ) -> JobInfoOutputProperties {
        return JobInfoOutputProperties(
            completedAt: completedAt ?? self.completedAt,
            errorMessage: errorMessage ?? self.errorMessage,
            id: id ?? self.id,
            name: name ?? self.name,
            progress: progress ?? self.progress,
            startedAt: startedAt ?? self.startedAt,
            status: status ?? self.status
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - TypesJobOutput
struct TypesJobOutput: Codable {
    let schema: String
    let definitions: JobOutputDefinitions
    let description: String
    let oneOf: [JobOutputOneOf]
    let title: String

    enum CodingKeys: String, CodingKey {
        case schema = "$schema"
        case definitions, description, oneOf, title
    }
}

// MARK: TypesJobOutput convenience initializers and mutators

extension TypesJobOutput {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(TypesJobOutput.self, from: data)
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
        schema: String? = nil,
        definitions: JobOutputDefinitions? = nil,
        description: String? = nil,
        oneOf: [JobOutputOneOf]? = nil,
        title: String? = nil
    ) -> TypesJobOutput {
        return TypesJobOutput(
            schema: schema ?? self.schema,
            definitions: definitions ?? self.definitions,
            description: description ?? self.description,
            oneOf: oneOf ?? self.oneOf,
            title: title ?? self.title
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - JobOutputDefinitions
struct JobOutputDefinitions: Codable {
    let duration: Duration
    let indexerMetrics: DefinitionsIndexerMetrics
    let indexerStats: DefinitionsIndexerStats

    enum CodingKeys: String, CodingKey {
        case duration = "Duration"
        case indexerMetrics = "IndexerMetrics"
        case indexerStats = "IndexerStats"
    }
}

// MARK: JobOutputDefinitions convenience initializers and mutators

extension JobOutputDefinitions {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(JobOutputDefinitions.self, from: data)
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
        duration: Duration? = nil,
        indexerMetrics: DefinitionsIndexerMetrics? = nil,
        indexerStats: DefinitionsIndexerStats? = nil
    ) -> JobOutputDefinitions {
        return JobOutputDefinitions(
            duration: duration ?? self.duration,
            indexerMetrics: indexerMetrics ?? self.indexerMetrics,
            indexerStats: indexerStats ?? self.indexerStats
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - LocationAddOutput
struct LocationAddOutput: Codable {
    let schema: String
    let description: String
    let properties: LocationAddOutputProperties
    let locationAddOutputRequired: [String]
    let title: String
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case schema = "$schema"
        case description, properties
        case locationAddOutputRequired = "required"
        case title, type
    }
}

// MARK: LocationAddOutput convenience initializers and mutators

extension LocationAddOutput {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(LocationAddOutput.self, from: data)
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
        schema: String? = nil,
        description: String? = nil,
        properties: LocationAddOutputProperties? = nil,
        locationAddOutputRequired: [String]? = nil,
        title: String? = nil,
        type: OutputType? = nil
    ) -> LocationAddOutput {
        return LocationAddOutput(
            schema: schema ?? self.schema,
            description: description ?? self.description,
            properties: properties ?? self.properties,
            locationAddOutputRequired: locationAddOutputRequired ?? self.locationAddOutputRequired,
            title: title ?? self.title,
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

// MARK: - LocationAddOutputProperties
struct LocationAddOutputProperties: Codable {
    let jobID: CompletedAt
    let locationID: JobID
    let name: ErrorMessage
    let path: Destination

    enum CodingKeys: String, CodingKey {
        case jobID = "job_id"
        case locationID = "location_id"
        case name, path
    }
}

// MARK: LocationAddOutputProperties convenience initializers and mutators

extension LocationAddOutputProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(LocationAddOutputProperties.self, from: data)
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
        jobID: CompletedAt? = nil,
        locationID: JobID? = nil,
        name: ErrorMessage? = nil,
        path: Destination? = nil
    ) -> LocationAddOutputProperties {
        return LocationAddOutputProperties(
            jobID: jobID ?? self.jobID,
            locationID: locationID ?? self.locationID,
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

// MARK: - TypesPathMapping
struct TypesPathMapping: Codable {
    let schema: String
    let description: String
    let properties: PathMappingProperties
    let pathMappingRequired: [String]
    let title: String
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case schema = "$schema"
        case description, properties
        case pathMappingRequired = "required"
        case title, type
    }
}

// MARK: TypesPathMapping convenience initializers and mutators

extension TypesPathMapping {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(TypesPathMapping.self, from: data)
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
        schema: String? = nil,
        description: String? = nil,
        properties: PathMappingProperties? = nil,
        pathMappingRequired: [String]? = nil,
        title: String? = nil,
        type: OutputType? = nil
    ) -> TypesPathMapping {
        return TypesPathMapping(
            schema: schema ?? self.schema,
            description: description ?? self.description,
            properties: properties ?? self.properties,
            pathMappingRequired: pathMappingRequired ?? self.pathMappingRequired,
            title: title ?? self.title,
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

// MARK: - TypesPerformanceMetrics
struct TypesPerformanceMetrics: Codable {
    let schema: String
    let definitions: IndexerMetricsDefinitions
    let description: String
    let properties: PerformanceMetricsProperties
    let performanceMetricsRequired: [String]
    let title: String
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case schema = "$schema"
        case definitions, description, properties
        case performanceMetricsRequired = "required"
        case title, type
    }
}

// MARK: TypesPerformanceMetrics convenience initializers and mutators

extension TypesPerformanceMetrics {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(TypesPerformanceMetrics.self, from: data)
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
        schema: String? = nil,
        definitions: IndexerMetricsDefinitions? = nil,
        description: String? = nil,
        properties: PerformanceMetricsProperties? = nil,
        performanceMetricsRequired: [String]? = nil,
        title: String? = nil,
        type: OutputType? = nil
    ) -> TypesPerformanceMetrics {
        return TypesPerformanceMetrics(
            schema: schema ?? self.schema,
            definitions: definitions ?? self.definitions,
            description: description ?? self.description,
            properties: properties ?? self.properties,
            performanceMetricsRequired: performanceMetricsRequired ?? self.performanceMetricsRequired,
            title: title ?? self.title,
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

// MARK: - Progress
struct Progress: Codable {
    let schema: String
    let definitions: GenericProgressDefinitions
    let description: String
    let oneOf: [ProgressOneOf]
    let title: String

    enum CodingKeys: String, CodingKey {
        case schema = "$schema"
        case definitions, description, oneOf, title
    }
}

// MARK: Progress convenience initializers and mutators

extension Progress {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(Progress.self, from: data)
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
        schema: String? = nil,
        definitions: GenericProgressDefinitions? = nil,
        description: String? = nil,
        oneOf: [ProgressOneOf]? = nil,
        title: String? = nil
    ) -> Progress {
        return Progress(
            schema: schema ?? self.schema,
            definitions: definitions ?? self.definitions,
            description: description ?? self.description,
            oneOf: oneOf ?? self.oneOf,
            title: title ?? self.title
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - ProgressOneOf
struct ProgressOneOf: Codable {
    let description: String
    let properties: HilariousProperties
    let oneOfRequired: [OneOfRequired]
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case description, properties
        case oneOfRequired = "required"
        case type
    }
}

// MARK: ProgressOneOf convenience initializers and mutators

extension ProgressOneOf {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(ProgressOneOf.self, from: data)
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
        description: String? = nil,
        properties: HilariousProperties? = nil,
        oneOfRequired: [OneOfRequired]? = nil,
        type: OutputType? = nil
    ) -> ProgressOneOf {
        return ProgressOneOf(
            description: description ?? self.description,
            properties: properties ?? self.properties,
            oneOfRequired: oneOfRequired ?? self.oneOfRequired,
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

// MARK: - HilariousProperties
struct HilariousProperties: Codable {
    let data: StickyData
    let type: TypeClass
}

// MARK: HilariousProperties convenience initializers and mutators

extension HilariousProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(HilariousProperties.self, from: data)
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
        data: StickyData? = nil,
        type: TypeClass? = nil
    ) -> HilariousProperties {
        return HilariousProperties(
            data: data ?? self.data,
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

enum StickyData: Codable {
    case bool(Bool)
    case fluffyData(FluffyData)

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let x = try? container.decode(Bool.self) {
            self = .bool(x)
            return
        }
        if let x = try? container.decode(FluffyData.self) {
            self = .fluffyData(x)
            return
        }
        throw DecodingError.typeMismatch(StickyData.self, DecodingError.Context(codingPath: decoder.codingPath, debugDescription: "Wrong type for StickyData"))
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .bool(let x):
            try container.encode(x)
        case .fluffyData(let x):
            try container.encode(x)
        }
    }
}

// MARK: - FluffyData
struct FluffyData: Codable {
    let properties: AmbitiousProperties?
    let dataRequired: [String]?
    let type: String?
    let format: Format?
    let ref: String?

    enum CodingKeys: String, CodingKey {
        case properties
        case dataRequired = "required"
        case type, format
        case ref = "$ref"
    }
}

// MARK: FluffyData convenience initializers and mutators

extension FluffyData {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(FluffyData.self, from: data)
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
        properties: AmbitiousProperties?? = nil,
        dataRequired: [String]?? = nil,
        type: String?? = nil,
        format: Format?? = nil,
        ref: String?? = nil
    ) -> FluffyData {
        return FluffyData(
            properties: properties ?? self.properties,
            dataRequired: dataRequired ?? self.dataRequired,
            type: type ?? self.type,
            format: format ?? self.format,
            ref: ref ?? self.ref
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - AmbitiousProperties
struct AmbitiousProperties: Codable {
    let current, total: CapacityFree
}

// MARK: AmbitiousProperties convenience initializers and mutators

extension AmbitiousProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(AmbitiousProperties.self, from: data)
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
        current: CapacityFree? = nil,
        total: CapacityFree? = nil
    ) -> AmbitiousProperties {
        return AmbitiousProperties(
            current: current ?? self.current,
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

// MARK: - TypesProgressCompletion
struct TypesProgressCompletion: Codable {
    let schema: String
    let description: String
    let properties: ProgressCompletionProperties
    let progressCompletionRequired: [String]
    let title: String
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case schema = "$schema"
        case description, properties
        case progressCompletionRequired = "required"
        case title, type
    }
}

// MARK: TypesProgressCompletion convenience initializers and mutators

extension TypesProgressCompletion {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(TypesProgressCompletion.self, from: data)
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
        schema: String? = nil,
        description: String? = nil,
        properties: ProgressCompletionProperties? = nil,
        progressCompletionRequired: [String]? = nil,
        title: String? = nil,
        type: OutputType? = nil
    ) -> TypesProgressCompletion {
        return TypesProgressCompletion(
            schema: schema ?? self.schema,
            description: description ?? self.description,
            properties: properties ?? self.properties,
            progressCompletionRequired: progressCompletionRequired ?? self.progressCompletionRequired,
            title: title ?? self.title,
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

// MARK: - TypesSDPath
struct TypesSDPath: Codable {
    let schema: String
    let description: String
    let oneOf: [SDPathOneOf]
    let title: String

    enum CodingKeys: String, CodingKey {
        case schema = "$schema"
        case description, oneOf, title
    }
}

// MARK: TypesSDPath convenience initializers and mutators

extension TypesSDPath {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(TypesSDPath.self, from: data)
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
        schema: String? = nil,
        description: String? = nil,
        oneOf: [SDPathOneOf]? = nil,
        title: String? = nil
    ) -> TypesSDPath {
        return TypesSDPath(
            schema: schema ?? self.schema,
            description: description ?? self.description,
            oneOf: oneOf ?? self.oneOf,
            title: title ?? self.title
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - SDPathBatch
struct SDPathBatch: Codable {
    let schema: String
    let definitions: SDPathBatchDefinitions
    let description: String
    let properties: SDPathBatchProperties
    let sdPathBatchRequired: [String]
    let title: String
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case schema = "$schema"
        case definitions, description, properties
        case sdPathBatchRequired = "required"
        case title, type
    }
}

// MARK: SDPathBatch convenience initializers and mutators

extension SDPathBatch {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(SDPathBatch.self, from: data)
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
        schema: String? = nil,
        definitions: SDPathBatchDefinitions? = nil,
        description: String? = nil,
        properties: SDPathBatchProperties? = nil,
        sdPathBatchRequired: [String]? = nil,
        title: String? = nil,
        type: OutputType? = nil
    ) -> SDPathBatch {
        return SDPathBatch(
            schema: schema ?? self.schema,
            definitions: definitions ?? self.definitions,
            description: description ?? self.description,
            properties: properties ?? self.properties,
            sdPathBatchRequired: sdPathBatchRequired ?? self.sdPathBatchRequired,
            title: title ?? self.title,
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

// MARK: - SDPathBatchDefinitions
struct SDPathBatchDefinitions: Codable {
    let sdPath: DefinitionsSDPath

    enum CodingKeys: String, CodingKey {
        case sdPath = "SdPath"
    }
}

// MARK: SDPathBatchDefinitions convenience initializers and mutators

extension SDPathBatchDefinitions {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(SDPathBatchDefinitions.self, from: data)
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
        sdPath: DefinitionsSDPath? = nil
    ) -> SDPathBatchDefinitions {
        return SDPathBatchDefinitions(
            sdPath: sdPath ?? self.sdPath
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - SDPathBatchProperties
struct SDPathBatchProperties: Codable {
    let paths: LibrariesClass
}

// MARK: SDPathBatchProperties convenience initializers and mutators

extension SDPathBatchProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(SDPathBatchProperties.self, from: data)
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
        paths: LibrariesClass? = nil
    ) -> SDPathBatchProperties {
        return SDPathBatchProperties(
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

// MARK: - TypesVolume
struct TypesVolume: Codable {
    let schema: String
    let definitions: VolumeDefinitions
    let description: String
    let properties: VolumeProperties
    let volumeRequired: [String]
    let title: String
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case schema = "$schema"
        case definitions, description, properties
        case volumeRequired = "required"
        case title, type
    }
}

// MARK: TypesVolume convenience initializers and mutators

extension TypesVolume {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(TypesVolume.self, from: data)
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
        schema: String? = nil,
        definitions: VolumeDefinitions? = nil,
        description: String? = nil,
        properties: VolumeProperties? = nil,
        volumeRequired: [String]? = nil,
        title: String? = nil,
        type: OutputType? = nil
    ) -> TypesVolume {
        return TypesVolume(
            schema: schema ?? self.schema,
            definitions: definitions ?? self.definitions,
            description: description ?? self.description,
            properties: properties ?? self.properties,
            volumeRequired: volumeRequired ?? self.volumeRequired,
            title: title ?? self.title,
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

// MARK: - VolumeDefinitions
struct VolumeDefinitions: Codable {
    let apfsContainer: DefinitionsApfsContainer
    let apfsVolumeInfo: DefinitionsApfsVolumeInfo
    let apfsVolumeRole: DefinitionsApfsVolumeRole
    let diskType: DefinitionsDiskType
    let fileSystem: DefinitionsApfsVolumeRole
    let mountType: DefinitionsDiskType
    let pathMapping: DefinitionsPathMapping
    let volumeFingerprint: ContainerID
    let volumeType: DefinitionsDiskType

    enum CodingKeys: String, CodingKey {
        case apfsContainer = "ApfsContainer"
        case apfsVolumeInfo = "ApfsVolumeInfo"
        case apfsVolumeRole = "ApfsVolumeRole"
        case diskType = "DiskType"
        case fileSystem = "FileSystem"
        case mountType = "MountType"
        case pathMapping = "PathMapping"
        case volumeFingerprint = "VolumeFingerprint"
        case volumeType = "VolumeType"
    }
}

// MARK: VolumeDefinitions convenience initializers and mutators

extension VolumeDefinitions {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(VolumeDefinitions.self, from: data)
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
        apfsContainer: DefinitionsApfsContainer? = nil,
        apfsVolumeInfo: DefinitionsApfsVolumeInfo? = nil,
        apfsVolumeRole: DefinitionsApfsVolumeRole? = nil,
        diskType: DefinitionsDiskType? = nil,
        fileSystem: DefinitionsApfsVolumeRole? = nil,
        mountType: DefinitionsDiskType? = nil,
        pathMapping: DefinitionsPathMapping? = nil,
        volumeFingerprint: ContainerID? = nil,
        volumeType: DefinitionsDiskType? = nil
    ) -> VolumeDefinitions {
        return VolumeDefinitions(
            apfsContainer: apfsContainer ?? self.apfsContainer,
            apfsVolumeInfo: apfsVolumeInfo ?? self.apfsVolumeInfo,
            apfsVolumeRole: apfsVolumeRole ?? self.apfsVolumeRole,
            diskType: diskType ?? self.diskType,
            fileSystem: fileSystem ?? self.fileSystem,
            mountType: mountType ?? self.mountType,
            pathMapping: pathMapping ?? self.pathMapping,
            volumeFingerprint: volumeFingerprint ?? self.volumeFingerprint,
            volumeType: volumeType ?? self.volumeType
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - TypesVolumeInfo
struct TypesVolumeInfo: Codable {
    let schema: String
    let description: String
    let properties: VolumeInfoProperties
    let volumeInfoRequired: [String]
    let title: String
    let type: OutputType

    enum CodingKeys: String, CodingKey {
        case schema = "$schema"
        case description, properties
        case volumeInfoRequired = "required"
        case title, type
    }
}

// MARK: TypesVolumeInfo convenience initializers and mutators

extension TypesVolumeInfo {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(TypesVolumeInfo.self, from: data)
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
        schema: String? = nil,
        description: String? = nil,
        properties: VolumeInfoProperties? = nil,
        volumeInfoRequired: [String]? = nil,
        title: String? = nil,
        type: OutputType? = nil
    ) -> TypesVolumeInfo {
        return TypesVolumeInfo(
            schema: schema ?? self.schema,
            description: description ?? self.description,
            properties: properties ?? self.properties,
            volumeInfoRequired: volumeInfoRequired ?? self.volumeInfoRequired,
            title: title ?? self.title,
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

// MARK: - Encode/decode helpers

class JSONNull: Codable, Hashable {

    public static func == (lhs: JSONNull, rhs: JSONNull) -> Bool {
            return true
    }

    public var hashValue: Int {
            return 0
    }

    public init() {}

    public required init(from decoder: Decoder) throws {
            let container = try decoder.singleValueContainer()
            if !container.decodeNil() {
                    throw DecodingError.typeMismatch(JSONNull.self, DecodingError.Context(codingPath: decoder.codingPath, debugDescription: "Wrong type for JSONNull"))
            }
    }

    public func encode(to encoder: Encoder) throws {
            var container = encoder.singleValueContainer()
            try container.encodeNil()
    }
}
