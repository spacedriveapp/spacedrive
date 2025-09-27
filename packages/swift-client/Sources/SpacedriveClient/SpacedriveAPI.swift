import Foundation

/// Libraries operations
public struct LibrariesAPI {
    private let client: SpacedriveClient

    init(client: SpacedriveClient) {
        self.client = client
    }

    /// Execute action: libraries.delete
    public func delete(_ input: LibraryDeleteInput) async throws -> LibraryDeleteOutput {
        return try await client.execute(
            input,
            method: "action:libraries.delete.input.v1",
            responseType: LibraryDeleteOutput.self
        )
    }

    /// Execute action: libraries.export
    public func export(_ input: LibraryExportInput) async throws -> LibraryExportOutput {
        return try await client.execute(
            input,
            method: "action:libraries.export.input.v1",
            responseType: LibraryExportOutput.self
        )
    }

    /// Execute action: libraries.rename
    public func rename(_ input: LibraryRenameInput) async throws -> LibraryRenameOutput {
        return try await client.execute(
            input,
            method: "action:libraries.rename.input.v1",
            responseType: LibraryRenameOutput.self
        )
    }

    /// Execute action: libraries.create
    public func create(_ input: LibraryCreateInput) async throws -> LibraryCreateOutput {
        return try await client.execute(
            input,
            method: "action:libraries.create.input.v1",
            responseType: LibraryCreateOutput.self
        )
    }

    /// Execute query: libraries.list
    public func list(_ input: ListLibrariesInput) async throws -> [LibraryInfo] {
        return try await client.execute(
            input,
            method: "query:libraries.list.v1",
            responseType: [LibraryInfo].self
        )
    }

    /// Execute query: libraries.info
    public func info(_ input: LibraryInfoQueryInput) async throws -> LibraryInfoOutput {
        return try await client.execute(
            input,
            method: "query:libraries.info.v1",
            responseType: LibraryInfoOutput.self
        )
    }

}

/// Media operations
public struct MediaAPI {
    private let client: SpacedriveClient

    init(client: SpacedriveClient) {
        self.client = client
    }

    /// Execute action: media.thumbnail
    public func thumbnail(_ input: ThumbnailInput) async throws -> JobReceipt {
        return try await client.execute(
            input,
            method: "action:media.thumbnail.input.v1",
            responseType: JobReceipt.self
        )
    }

}

/// Jobs operations
public struct JobsAPI {
    private let client: SpacedriveClient

    init(client: SpacedriveClient) {
        self.client = client
    }

    /// Execute action: jobs.resume
    public func resume(_ input: JobResumeInput) async throws -> JobResumeOutput {
        return try await client.execute(
            input,
            method: "action:jobs.resume.input.v1",
            responseType: JobResumeOutput.self
        )
    }

    /// Execute action: jobs.pause
    public func pause(_ input: JobPauseInput) async throws -> JobPauseOutput {
        return try await client.execute(
            input,
            method: "action:jobs.pause.input.v1",
            responseType: JobPauseOutput.self
        )
    }

    /// Execute action: jobs.cancel
    public func cancel(_ input: JobCancelInput) async throws -> JobCancelOutput {
        return try await client.execute(
            input,
            method: "action:jobs.cancel.input.v1",
            responseType: JobCancelOutput.self
        )
    }

    /// Execute query: jobs.list
    public func list(_ input: JobListInput) async throws -> JobListOutput {
        return try await client.execute(
            input,
            method: "query:jobs.list.v1",
            responseType: JobListOutput.self
        )
    }

    /// Execute query: jobs.info
    public func info(_ input: JobInfoQueryInput) async throws -> JobInfoOutput {
        return try await client.execute(
            input,
            method: "query:jobs.info.v1",
            responseType: JobInfoOutput.self
        )
    }

}

/// Search operations
public struct SearchAPI {
    private let client: SpacedriveClient

    init(client: SpacedriveClient) {
        self.client = client
    }

    /// Execute query: search.files
    public func files(_ input: FileSearchInput) async throws -> FileSearchOutput {
        return try await client.execute(
            input,
            method: "query:search.files.v1",
            responseType: FileSearchOutput.self
        )
    }

}

/// Files operations
public struct FilesAPI {
    private let client: SpacedriveClient

    init(client: SpacedriveClient) {
        self.client = client
    }

    /// Execute action: files.copy
    public func copy(_ input: FileCopyInput) async throws -> JobReceipt {
        return try await client.execute(
            input,
            method: "action:files.copy.input.v1",
            responseType: JobReceipt.self
        )
    }

    /// Execute query: files.by_path
    public func byPath(_ input: FileByPathQuery) async throws -> File {
        return try await client.execute(
            input,
            method: "query:files.by_path.v1",
            responseType: File.self
        )
    }

    /// Execute query: files.by_id
    public func byId(_ input: FileByIdQuery) async throws -> File {
        return try await client.execute(
            input,
            method: "query:files.by_id.v1",
            responseType: File.self
        )
    }

    /// Execute query: files.unique_to_location
    public func uniqueToLocation(_ input: UniqueToLocationInput) async throws -> UniqueToLocationOutput {
        return try await client.execute(
            input,
            method: "query:files.unique_to_location.v1",
            responseType: UniqueToLocationOutput.self
        )
    }

    /// Execute query: files.directory_listing
    public func directoryListing(_ input: DirectoryListingInput) async throws -> DirectoryListingOutput {
        return try await client.execute(
            input,
            method: "query:files.directory_listing.v1",
            responseType: DirectoryListingOutput.self
        )
    }

}

/// Locations operations
public struct LocationsAPI {
    private let client: SpacedriveClient

    init(client: SpacedriveClient) {
        self.client = client
    }

    /// Execute action: locations.add
    public func add(_ input: LocationAddInput) async throws -> LocationAddOutput {
        return try await client.execute(
            input,
            method: "action:locations.add.input.v1",
            responseType: LocationAddOutput.self
        )
    }

    /// Execute action: locations.remove
    public func remove(_ input: LocationRemoveInput) async throws -> LocationRemoveOutput {
        return try await client.execute(
            input,
            method: "action:locations.remove.input.v1",
            responseType: LocationRemoveOutput.self
        )
    }

    /// Execute action: locations.rescan
    public func rescan(_ input: LocationRescanInput) async throws -> LocationRescanOutput {
        return try await client.execute(
            input,
            method: "action:locations.rescan.input.v1",
            responseType: LocationRescanOutput.self
        )
    }

    /// Execute query: locations.list
    public func list(_ input: LocationsListQueryInput) async throws -> LocationsListOutput {
        return try await client.execute(
            input,
            method: "query:locations.list.v1",
            responseType: LocationsListOutput.self
        )
    }

}

/// Indexing operations
public struct IndexingAPI {
    private let client: SpacedriveClient

    init(client: SpacedriveClient) {
        self.client = client
    }

    /// Execute action: indexing.start
    public func start(_ input: IndexInput) async throws -> JobReceipt {
        return try await client.execute(
            input,
            method: "action:indexing.start.input.v1",
            responseType: JobReceipt.self
        )
    }

}

/// Network operations
public struct NetworkAPI {
    private let client: SpacedriveClient

    init(client: SpacedriveClient) {
        self.client = client
    }

    /// Execute action: network.device.revoke
    public func deviceRevoke(_ input: DeviceRevokeInput) async throws -> DeviceRevokeOutput {
        return try await client.execute(
            input,
            method: "action:network.device.revoke.input.v1",
            responseType: DeviceRevokeOutput.self
        )
    }

    /// Execute action: network.pair.cancel
    public func pairCancel(_ input: PairCancelInput) async throws -> PairCancelOutput {
        return try await client.execute(
            input,
            method: "action:network.pair.cancel.input.v1",
            responseType: PairCancelOutput.self
        )
    }

    /// Execute action: network.stop
    public func stop(_ input: NetworkStopInput) async throws -> NetworkStopOutput {
        return try await client.execute(
            input,
            method: "action:network.stop.input.v1",
            responseType: NetworkStopOutput.self
        )
    }

    /// Execute action: network.start
    public func start(_ input: NetworkStartInput) async throws -> NetworkStartOutput {
        return try await client.execute(
            input,
            method: "action:network.start.input.v1",
            responseType: NetworkStartOutput.self
        )
    }

    /// Execute action: network.pair.generate
    public func pairGenerate(_ input: PairGenerateInput) async throws -> PairGenerateOutput {
        return try await client.execute(
            input,
            method: "action:network.pair.generate.input.v1",
            responseType: PairGenerateOutput.self
        )
    }

    /// Execute action: network.spacedrop.send
    public func spacedropSend(_ input: SpacedropSendInput) async throws -> SpacedropSendOutput {
        return try await client.execute(
            input,
            method: "action:network.spacedrop.send.input.v1",
            responseType: SpacedropSendOutput.self
        )
    }

    /// Execute action: network.pair.join
    public func pairJoin(_ input: PairJoinInput) async throws -> PairJoinOutput {
        return try await client.execute(
            input,
            method: "action:network.pair.join.input.v1",
            responseType: PairJoinOutput.self
        )
    }

    /// Execute query: network.status
    public func status(_ input: NetworkStatusQueryInput) async throws -> NetworkStatus {
        return try await client.execute(
            input,
            method: "query:network.status.v1",
            responseType: NetworkStatus.self
        )
    }

    /// Execute query: network.devices
    public func devices(_ input: ListDevicesInput) async throws -> [DeviceInfoLite] {
        return try await client.execute(
            input,
            method: "query:network.devices.v1",
            responseType: [DeviceInfoLite].self
        )
    }

    /// Execute query: network.pair.status
    public func pairStatus(_ input: PairStatusQueryInput) async throws -> PairStatusOutput {
        return try await client.execute(
            input,
            method: "query:network.pair.status.v1",
            responseType: PairStatusOutput.self
        )
    }

}

/// Core operations
public struct CoreAPI {
    private let client: SpacedriveClient

    init(client: SpacedriveClient) {
        self.client = client
    }

    /// Execute query: core.status
    public func status(_ input: Empty) async throws -> CoreStatus {
        return try await client.execute(
            input,
            method: "query:core.status.v1",
            responseType: CoreStatus.self
        )
    }

}

/// Tags operations
public struct TagsAPI {
    private let client: SpacedriveClient

    init(client: SpacedriveClient) {
        self.client = client
    }

    /// Execute action: tags.apply
    public func apply(_ input: ApplyTagsInput) async throws -> ApplyTagsOutput {
        return try await client.execute(
            input,
            method: "action:tags.apply.input.v1",
            responseType: ApplyTagsOutput.self
        )
    }

    /// Execute action: tags.create
    public func create(_ input: CreateTagInput) async throws -> CreateTagOutput {
        return try await client.execute(
            input,
            method: "action:tags.create.input.v1",
            responseType: CreateTagOutput.self
        )
    }

    /// Execute query: tags.search
    public func search(_ input: SearchTagsInput) async throws -> SearchTagsOutput {
        return try await client.execute(
            input,
            method: "query:tags.search.v1",
            responseType: SearchTagsOutput.self
        )
    }

}

/// Volumes operations
public struct VolumesAPI {
    private let client: SpacedriveClient

    init(client: SpacedriveClient) {
        self.client = client
    }

    /// Execute action: volumes.speed_test
    public func speedTest(_ input: VolumeSpeedTestInput) async throws -> VolumeSpeedTestOutput {
        return try await client.execute(
            input,
            method: "action:volumes.speed_test.input.v1",
            responseType: VolumeSpeedTestOutput.self
        )
    }

    /// Execute action: volumes.track
    public func track(_ input: VolumeTrackInput) async throws -> VolumeTrackOutput {
        return try await client.execute(
            input,
            method: "action:volumes.track.input.v1",
            responseType: VolumeTrackOutput.self
        )
    }

    /// Execute action: volumes.untrack
    public func untrack(_ input: VolumeUntrackInput) async throws -> VolumeUntrackOutput {
        return try await client.execute(
            input,
            method: "action:volumes.untrack.input.v1",
            responseType: VolumeUntrackOutput.self
        )
    }

}

