import type {VolumeState} from '@sd/ts-client';

/**
 * What stands between the library and a source's files right now, as the
 * sidebar, the Storage page and a source's detail all say it.
 *
 * `locked` and `unmounted` come from the volume index's state of the drive
 * under the source; `offline` is a detached source the index has no state
 * for (a drive that vanished, a replica whose owner is away). A mounted
 * drive and an adapter, whose origin is a service, are `available`.
 */
export type SourceAvailability = 'available' | 'unmounted' | 'locked' | 'offline';

export function sourceAvailability(source: {
	attached: boolean;
	volume_state?: VolumeState | null;
}): SourceAvailability {
	switch (source.volume_state) {
		case 'Locked':
			return 'locked';
		case 'Unmounted':
			return 'unmounted';
		case 'Mounted':
			return 'available';
		default:
			return source.attached ? 'available' : 'offline';
	}
}

/** The badge a row or card wears, or nothing while the source is available. */
export function availabilityBadge(
	availability: SourceAvailability
): string | undefined {
	switch (availability) {
		case 'locked':
			return 'Locked';
		case 'unmounted':
			return 'Unmounted';
		case 'offline':
			return 'Offline';
		default:
			return undefined;
	}
}

/**
 * One sentence saying what the state means for the files, naming the
 * volume when the caller knows it. The index stays browsable in every
 * case: a locked or unmounted source still lists what it indexed.
 */
export function availabilityDescription(
	availability: SourceAvailability,
	volumeName: string | undefined
): string | undefined {
	const volume = volumeName ?? 'Its volume';
	switch (availability) {
		case 'locked':
			return `${volume} is locked: its encryption key is not loaded. Files stay listed from the last index and cannot be opened until the key is loaded.`;
		case 'unmounted':
			return `${volume} is not mounted. Files stay listed from the last index and cannot be opened until it mounts.`;
		case 'offline':
			return 'The drive is not connected. Files stay listed from the last index.';
		default:
			return undefined;
	}
}
