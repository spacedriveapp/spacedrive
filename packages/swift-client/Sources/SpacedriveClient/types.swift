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
    let title, type: String

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
        type: String? = nil
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
    let deviceInfo: Status
    let libraries: Paths
    let libraryCount: Count
    let network, services, system: Status
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
        deviceInfo: Status? = nil,
        libraries: Paths? = nil,
        libraryCount: Count? = nil,
        network: Status? = nil,
        services: Status? = nil,
        system: Status? = nil,
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
    let type: String
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
        type: String? = nil
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

// MARK: - Status
struct Status: Codable {
    let ref: String

    enum CodingKeys: String, CodingKey {
        case ref = "$ref"
    }
}

// MARK: Status convenience initializers and mutators

extension Status {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(Status.self, from: data)
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
    ) -> Status {
        return Status(
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

// MARK: - Paths
struct Paths: Codable {
    let items: Status
    let type: String
}

// MARK: Paths convenience initializers and mutators

extension Paths {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(Paths.self, from: data)
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
        items: Status? = nil,
        type: String? = nil
    ) -> Paths {
        return Paths(
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

// MARK: - Count
struct Count: Codable {
    let format: String
    let minimum: Double
    let type: String
}

// MARK: Count convenience initializers and mutators

extension Count {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(Count.self, from: data)
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
        format: String? = nil,
        minimum: Double? = nil,
        type: String? = nil
    ) -> Count {
        return Count(
            format: format ?? self.format,
            minimum: minimum ?? self.minimum,
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

enum QueryType: String, Codable {
    case query = "query"
}

// MARK: - TypesClass
struct TypesClass: Codable {
    let fileCopyActionOutput: FileCopyActionOutput
    let jobInfoOutput: JobInfoOutput
    let locationAddOutput: LocationAddOutput
    let sdPath: TypesSDPath
    let sdPathBatch: SDPathBatch

    enum CodingKeys: String, CodingKey {
        case fileCopyActionOutput = "FileCopyActionOutput"
        case jobInfoOutput = "JobInfoOutput"
        case locationAddOutput = "LocationAddOutput"
        case sdPath = "SdPath"
        case sdPathBatch = "SdPathBatch"
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
        fileCopyActionOutput: FileCopyActionOutput? = nil,
        jobInfoOutput: JobInfoOutput? = nil,
        locationAddOutput: LocationAddOutput? = nil,
        sdPath: TypesSDPath? = nil,
        sdPathBatch: SDPathBatch? = nil
    ) -> TypesClass {
        return TypesClass(
            fileCopyActionOutput: fileCopyActionOutput ?? self.fileCopyActionOutput,
            jobInfoOutput: jobInfoOutput ?? self.jobInfoOutput,
            locationAddOutput: locationAddOutput ?? self.locationAddOutput,
            sdPath: sdPath ?? self.sdPath,
            sdPathBatch: sdPathBatch ?? self.sdPathBatch
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
    let title, type: String

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
        type: String? = nil
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
    let sourcesCount: Count

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
        sourcesCount: Count? = nil
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

// MARK: - JobID
struct JobID: Codable {
    let format, type: String
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
        format: String? = nil,
        type: String? = nil
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

// MARK: - JobInfoOutput
struct JobInfoOutput: Codable {
    let schema: String
    let definitions: JobInfoOutputDefinitions
    let properties: JobInfoOutputProperties
    let jobInfoOutputRequired: [String]
    let title, type: String

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
        type: String? = nil
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
    let jobStatus: JobStatus

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
        jobStatus: JobStatus? = nil
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

// MARK: - JobStatus
struct JobStatus: Codable {
    let description: String
    let oneOf: [JobStatusOneOf]
}

// MARK: JobStatus convenience initializers and mutators

extension JobStatus {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(JobStatus.self, from: data)
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
        oneOf: [JobStatusOneOf]? = nil
    ) -> JobStatus {
        return JobStatus(
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

// MARK: - JobStatusOneOf
struct JobStatusOneOf: Codable {
    let description: String
    let oneOfEnum: [String]
    let type: String

    enum CodingKeys: String, CodingKey {
        case description
        case oneOfEnum = "enum"
        case type
    }
}

// MARK: JobStatusOneOf convenience initializers and mutators

extension JobStatusOneOf {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(JobStatusOneOf.self, from: data)
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
        type: String? = nil
    ) -> JobStatusOneOf {
        return JobStatusOneOf(
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

// MARK: - JobInfoOutputProperties
struct JobInfoOutputProperties: Codable {
    let completedAt: CompletedAt
    let errorMessage: ErrorMessage
    let id: JobID
    let name: Destination
    let progress, startedAt: JobID
    let status: Status

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
        status: Status? = nil
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

// MARK: - CompletedAt
struct CompletedAt: Codable {
    let format: String
    let type: [String]
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
        format: String? = nil,
        type: [String]? = nil
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

// MARK: - ErrorMessage
struct ErrorMessage: Codable {
    let type: [String]
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
        type: [String]? = nil
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

// MARK: - LocationAddOutput
struct LocationAddOutput: Codable {
    let schema: String
    let description: String
    let properties: LocationAddOutputProperties
    let locationAddOutputRequired: [String]
    let title, type: String

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
        type: String? = nil
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

// MARK: - SDPathOneOf
struct SDPathOneOf: Codable {
    let additionalProperties: Bool
    let description: String
    let properties: OneOfProperties
    let oneOfRequired: [String]
    let type: String

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
        properties: OneOfProperties? = nil,
        oneOfRequired: [String]? = nil,
        type: String? = nil
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

// MARK: - OneOfProperties
struct OneOfProperties: Codable {
    let physical: Physical?
    let content: Content?

    enum CodingKeys: String, CodingKey {
        case physical = "Physical"
        case content = "Content"
    }
}

// MARK: OneOfProperties convenience initializers and mutators

extension OneOfProperties {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(OneOfProperties.self, from: data)
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
    ) -> OneOfProperties {
        return OneOfProperties(
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
    let type: String

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
        type: String? = nil
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
    let contentID: ID

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
        contentID: ID? = nil
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

// MARK: - ID
struct ID: Codable {
    let description, format, type: String
}

// MARK: ID convenience initializers and mutators

extension ID {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(ID.self, from: data)
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
        format: String? = nil,
        type: String? = nil
    ) -> ID {
        return ID(
            description: description ?? self.description,
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

// MARK: - Physical
struct Physical: Codable {
    let properties: PhysicalProperties
    let physicalRequired: [String]
    let type: String

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
        type: String? = nil
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
    let deviceID: ID
    let path: Path

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
        deviceID: ID? = nil,
        path: Path? = nil
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

// MARK: - Path
struct Path: Codable {
    let description, type: String
}

// MARK: Path convenience initializers and mutators

extension Path {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(Path.self, from: data)
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
        type: String? = nil
    ) -> Path {
        return Path(
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

// MARK: - SDPathBatch
struct SDPathBatch: Codable {
    let schema: String
    let definitions: SDPathBatchDefinitions
    let description: String
    let properties: SDPathBatchProperties
    let sdPathBatchRequired: [String]
    let title, type: String

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
        type: String? = nil
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

// MARK: - SDPathBatchProperties
struct SDPathBatchProperties: Codable {
    let paths: Paths
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
        paths: Paths? = nil
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
