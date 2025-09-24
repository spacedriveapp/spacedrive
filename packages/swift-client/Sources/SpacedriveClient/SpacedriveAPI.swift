import Foundation

/// Network operations
public struct NetworkAPI {
    private let client: SpacedriveClient

    init(client: SpacedriveClient) {
        self.client = client
    }

    /// Execute action: network.device.revoke
    public func deviceRevoke(_ input: NetworkDeviceRevokeInput) async throws -> NetworkDeviceRevokeOutput {
        return try await client.execute(
            input,
            method: "action:network.device.revoke.input.v1",
            responseType: NetworkDeviceRevokeOutput.self
        )
    }

    /// Execute action: network.pair.join
    public func pairJoin(_ input: NetworkPairJoinInput) async throws -> NetworkPairJoinOutput {
        return try await client.execute(
            input,
            method: "action:network.pair.join.input.v1",
            responseType: NetworkPairJoinOutput.self
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
    public func pairGenerate(_ input: NetworkPairGenerateInput) async throws -> NetworkPairGenerateOutput {
        return try await client.execute(
            input,
            method: "action:network.pair.generate.input.v1",
            responseType: NetworkPairGenerateOutput.self
        )
    }

    /// Execute action: network.pair.cancel
    public func pairCancel(_ input: NetworkPairCancelInput) async throws -> NetworkPairCancelOutput {
        return try await client.execute(
            input,
            method: "action:network.pair.cancel.input.v1",
            responseType: NetworkPairCancelOutput.self
        )
    }

    /// Execute action: network.spacedrop.send
    public func spacedropSend(_ input: NetworkSpacedropSendInput) async throws -> NetworkSpacedropSendOutput {
        return try await client.execute(
            input,
            method: "action:network.spacedrop.send.input.v1",
            responseType: NetworkSpacedropSendOutput.self
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

    /// Execute query: network.devices
    public func devices(_ input: NetworkDevicesInput) async throws -> NetworkDevicesOutput {
        return try await client.execute(
            input,
            method: "query:network.devices.v1",
            responseType: NetworkDevicesOutput.self
        )
    }

    /// Execute query: network.status
    public func status(_ input: NetworkStatusInput) async throws -> NetworkStatusOutput {
        return try await client.execute(
            input,
            method: "query:network.status.v1",
            responseType: NetworkStatusOutput.self
        )
    }

    /// Execute query: network.pair.status
    public func pairStatus(_ input: NetworkPairStatusInput) async throws -> NetworkPairStatusOutput {
        return try await client.execute(
            input,
            method: "query:network.pair.status.v1",
            responseType: NetworkPairStatusOutput.self
        )
    }

}

/// Libraries operations
public struct LibrariesAPI {
    private let client: SpacedriveClient

    init(client: SpacedriveClient) {
        self.client = client
    }

    /// Execute action: libraries.delete
    public func delete(_ input: LibrariesDeleteInput) async throws -> LibrariesDeleteOutput {
        return try await client.execute(
            input,
            method: "action:libraries.delete.input.v1",
            responseType: LibrariesDeleteOutput.self
        )
    }

    /// Execute action: libraries.rename
    public func rename(_ input: LibrariesRenameInput) async throws -> LibrariesRenameOutput {
        return try await client.execute(
            input,
            method: "action:libraries.rename.input.v1",
            responseType: LibrariesRenameOutput.self
        )
    }

    /// Execute action: libraries.export
    public func export(_ input: LibrariesExportInput) async throws -> LibrariesExportOutput {
        return try await client.execute(
            input,
            method: "action:libraries.export.input.v1",
            responseType: LibrariesExportOutput.self
        )
    }

    /// Execute action: libraries.create
    public func create(_ input: LibrariesCreateInput) async throws -> LibrariesCreateOutput {
        return try await client.execute(
            input,
            method: "action:libraries.create.input.v1",
            responseType: LibrariesCreateOutput.self
        )
    }

    /// Execute query: libraries.info
    public func info(_ input: LibrariesInfoInput) async throws -> LibrariesInfoOutput {
        return try await client.execute(
            input,
            method: "query:libraries.info.v1",
            responseType: LibrariesInfoOutput.self
        )
    }

    /// Execute query: libraries.list
    public func list(_ input: LibrariesListInput) async throws -> LibrariesListOutput {
        return try await client.execute(
            input,
            method: "query:libraries.list.v1",
            responseType: LibrariesListOutput.self
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
    public func start(_ input: IndexingStartInput) async throws -> IndexingStartOutput {
        return try await client.execute(
            input,
            method: "action:indexing.start.input.v1",
            responseType: IndexingStartOutput.self
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
    public func status(_ input: CoreStatusInput) async throws -> CoreStatusOutput {
        return try await client.execute(
            input,
            method: "query:core.status.v1",
            responseType: CoreStatusOutput.self
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
    public func add(_ input: LocationsAddInput) async throws -> LocationsAddOutput {
        return try await client.execute(
            input,
            method: "action:locations.add.input.v1",
            responseType: LocationsAddOutput.self
        )
    }

    /// Execute action: locations.remove
    public func remove(_ input: LocationsRemoveInput) async throws -> LocationsRemoveOutput {
        return try await client.execute(
            input,
            method: "action:locations.remove.input.v1",
            responseType: LocationsRemoveOutput.self
        )
    }

    /// Execute action: locations.rescan
    public func rescan(_ input: LocationsRescanInput) async throws -> LocationsRescanOutput {
        return try await client.execute(
            input,
            method: "action:locations.rescan.input.v1",
            responseType: LocationsRescanOutput.self
        )
    }

    /// Execute query: locations.list
    public func list(_ input: LocationsListInput) async throws -> LocationsListOutput {
        return try await client.execute(
            input,
            method: "query:locations.list.v1",
            responseType: LocationsListOutput.self
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
    public func files(_ input: SearchFilesInput) async throws -> SearchFilesOutput {
        return try await client.execute(
            input,
            method: "query:search.files.v1",
            responseType: SearchFilesOutput.self
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
    public func apply(_ input: TagsApplyInput) async throws -> TagsApplyOutput {
        return try await client.execute(
            input,
            method: "action:tags.apply.input.v1",
            responseType: TagsApplyOutput.self
        )
    }

    /// Execute action: tags.create
    public func create(_ input: TagsCreateInput) async throws -> TagsCreateOutput {
        return try await client.execute(
            input,
            method: "action:tags.create.input.v1",
            responseType: TagsCreateOutput.self
        )
    }

    /// Execute query: tags.search
    public func search(_ input: TagsSearchInput) async throws -> TagsSearchOutput {
        return try await client.execute(
            input,
            method: "query:tags.search.v1",
            responseType: TagsSearchOutput.self
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
    public func copy(_ input: FilesCopyInput) async throws -> FilesCopyOutput {
        return try await client.execute(
            input,
            method: "action:files.copy.input.v1",
            responseType: FilesCopyOutput.self
        )
    }

    /// Execute action: files.validation
    public func validation(_ input: FilesValidationInput) async throws -> FilesValidationOutput {
        return try await client.execute(
            input,
            method: "action:files.validation.input.v1",
            responseType: FilesValidationOutput.self
        )
    }

    /// Execute action: files.delete
    public func delete(_ input: FilesDeleteInput) async throws -> FilesDeleteOutput {
        return try await client.execute(
            input,
            method: "action:files.delete.input.v1",
            responseType: FilesDeleteOutput.self
        )
    }

    /// Execute action: files.duplicate_detection
    public func duplicateDetection(_ input: FilesDuplicateDetectionInput) async throws -> FilesDuplicateDetectionOutput {
        return try await client.execute(
            input,
            method: "action:files.duplicate_detection.input.v1",
            responseType: FilesDuplicateDetectionOutput.self
        )
    }

}

/// Jobs operations
public struct JobsAPI {
    private let client: SpacedriveClient

    init(client: SpacedriveClient) {
        self.client = client
    }

    /// Execute action: jobs.pause
    public func pause(_ input: JobsPauseInput) async throws -> JobsPauseOutput {
        return try await client.execute(
            input,
            method: "action:jobs.pause.input.v1",
            responseType: JobsPauseOutput.self
        )
    }

    /// Execute action: jobs.cancel
    public func cancel(_ input: JobsCancelInput) async throws -> JobsCancelOutput {
        return try await client.execute(
            input,
            method: "action:jobs.cancel.input.v1",
            responseType: JobsCancelOutput.self
        )
    }

    /// Execute action: jobs.resume
    public func resume(_ input: JobsResumeInput) async throws -> JobsResumeOutput {
        return try await client.execute(
            input,
            method: "action:jobs.resume.input.v1",
            responseType: JobsResumeOutput.self
        )
    }

    /// Execute query: jobs.info
    public func info(_ input: JobsInfoInput) async throws -> JobsInfoOutput {
        return try await client.execute(
            input,
            method: "query:jobs.info.v1",
            responseType: JobsInfoOutput.self
        )
    }

    /// Execute query: jobs.list
    public func list(_ input: JobsListInput) async throws -> JobsListOutput {
        return try await client.execute(
            input,
            method: "query:jobs.list.v1",
            responseType: JobsListOutput.self
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
    public func thumbnail(_ input: MediaThumbnailInput) async throws -> MediaThumbnailOutput {
        return try await client.execute(
            input,
            method: "action:media.thumbnail.input.v1",
            responseType: MediaThumbnailOutput.self
        )
    }

}

/// Volumes operations
public struct VolumesAPI {
    private let client: SpacedriveClient

    init(client: SpacedriveClient) {
        self.client = client
    }

    /// Execute action: volumes.untrack
    public func untrack(_ input: VolumesUntrackInput) async throws -> VolumesUntrackOutput {
        return try await client.execute(
            input,
            method: "action:volumes.untrack.input.v1",
            responseType: VolumesUntrackOutput.self
        )
    }

    /// Execute action: volumes.track
    public func track(_ input: VolumesTrackInput) async throws -> VolumesTrackOutput {
        return try await client.execute(
            input,
            method: "action:volumes.track.input.v1",
            responseType: VolumesTrackOutput.self
        )
    }

    /// Execute action: volumes.speed_test
    public func speedTest(_ input: VolumesSpeedTestInput) async throws -> VolumesSpeedTestOutput {
        return try await client.execute(
            input,
            method: "action:volumes.speed_test.input.v1",
            responseType: VolumesSpeedTestOutput.self
        )
    }

}

