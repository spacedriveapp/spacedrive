/**
 * Event filtering utilities
 *
 * Extracts event variant names from the auto-generated Event type
 * to avoid hardcoding them in subscription requests.
 */

import type { Event } from "./generated/types";

/**
 * Extract event variant name from an Event union member
 */
type ExtractEventVariant<T> = T extends string
	? T
	: T extends Record<string, any>
		? keyof T extends string
			? keyof T
			: never
		: never;

/**
 * All possible event variant names extracted from the Event type
 */
export type EventVariant = ExtractEventVariant<Event>;

/**
 * Default event subscription list - excludes noisy events
 *
 * Subscribes to all lifecycle events but filters out:
 * - LogMessage: Too spammy (every INFO log becomes an event)
 * - JobProgress: Too frequent (use polling for job progress instead)
 * - IndexingProgress: Too frequent (use polling for indexing status)
 */
export const DEFAULT_EVENT_SUBSCRIPTION: EventVariant[] = [
	// Core lifecycle
	"CoreStarted",
	"CoreShutdown",
	// Library events
	"LibraryCreated",
	"LibraryOpened",
	"LibraryClosed",
	"LibraryDeleted",
	"LibraryStatisticsUpdated",
	// Entry events
	"EntryCreated",
	"EntryModified",
	"EntryDeleted",
	"EntryMoved",
	// Raw filesystem changes
	"FsRawChange",
	// Volume events
	"VolumeAdded",
	"VolumeRemoved",
	"VolumeUpdated",
	"VolumeSpeedTested",
	"VolumeMountChanged",
	"VolumeError",
	// Job lifecycle
	"JobQueued",
	"JobStarted",
	"JobProgress",
	"JobCompleted",
	"JobFailed",
	"JobCancelled",
	"JobPaused",
	"JobResumed",
	// Indexing lifecycle (no progress spam)
	"IndexingStarted",
	"IndexingCompleted",
	"IndexingFailed",
	// Device events
	"DeviceConnected",
	"DeviceDisconnected",
	// Resource events (normalized cache updates)
	"ResourceChanged",
	"ResourceChangedBatch",
	"ResourceDeleted",
	// Legacy compatibility events
	"LocationAdded",
	"LocationRemoved",
	"FilesIndexed",
	"ThumbnailsGenerated",
	"FileOperationCompleted",
	"FilesModified",
];

/**
 * Noisy events that are excluded from the default subscription
 */
export const NOISY_EVENTS: EventVariant[] = [
	"LogMessage",
	"IndexingProgress",
];
