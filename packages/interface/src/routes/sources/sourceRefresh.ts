import type { Event, SdPath } from "@sd/ts-client";

export function sourceFileEventFilter(libraryId: string, root: string | null) {
	const pathScope: SdPath | undefined = root
		? { Physical: { device_slug: "local", path: root } }
		: undefined;
	return {
		library_id: libraryId,
		resource_type: "file",
		event_types: [
			"ResourceChanged",
			"ResourceChangedBatch",
			"ResourceDeleted",
		],
		...(pathScope
			? { path_scope: pathScope, include_descendants: true }
			: {}),
	};
}

export function shouldRefreshSourceForEvent(
	event: Event,
	volumeFingerprint?: string,
	volumeId?: string | null,
): boolean {
	if (typeof event === "string") return false;
	if ("ResourceChanged" in event)
		return event.ResourceChanged.resource_type === "file";
	if ("ResourceChangedBatch" in event)
		return event.ResourceChangedBatch.resource_type === "file";
	if ("ResourceDeleted" in event)
		return event.ResourceDeleted.resource_type === "file";
	if ("JobCompleted" in event)
		return (
			event.JobCompleted.job_type === "indexer" ||
			event.JobCompleted.job_type === "source_sync"
		);
	// An absent drive may not be in volumes.list until it returns, so its
	// fingerprint is unknown. Refresh this source on volume changes to learn it.
	if (!volumeFingerprint) {
		return (
			!!volumeId &&
			("VolumeAdded" in event ||
				"VolumeRemoved" in event ||
				"VolumeUpdated" in event ||
				"VolumeMountChanged" in event)
		);
	}
	if ("VolumeAdded" in event)
		return event.VolumeAdded.fingerprint === volumeFingerprint;
	if ("VolumeRemoved" in event)
		return event.VolumeRemoved.fingerprint === volumeFingerprint;
	if ("VolumeUpdated" in event)
		return event.VolumeUpdated.fingerprint === volumeFingerprint;
	if ("VolumeMountChanged" in event)
		return event.VolumeMountChanged.fingerprint === volumeFingerprint;
	return false;
}

export function sourceListQueryKey(libraryId: string) {
	return ["sources.list", libraryId] as const;
}

export function isSourceMetadataEvent(event: Event): boolean {
	return (
		typeof event !== "string" &&
		("JobCompleted" in event ||
			"VolumeAdded" in event ||
			"VolumeRemoved" in event ||
			"VolumeUpdated" in event ||
			"VolumeMountChanged" in event)
	);
}
