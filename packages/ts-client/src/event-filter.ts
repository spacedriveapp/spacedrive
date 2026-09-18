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
 * Subscribes to all lifecycle events but filters out LogMessage, which is
 * too spammy: every INFO log becomes an event.
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
	// Device events
	"DeviceConnected",
	"DeviceDisconnected",
	// Resource events (normalized cache updates)
	"ResourceChanged",
	"ResourceChangedBatch",
	"ResourceDeleted",
];
