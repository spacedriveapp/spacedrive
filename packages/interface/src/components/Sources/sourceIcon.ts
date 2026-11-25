import DatabaseIcon from '@sd/assets/icons/Database.webp';
import DriveIcon from '@sd/assets/icons/Drive.webp';
import HDDIcon from '@sd/assets/icons/HDD.webp';
import ServerIcon from '@sd/assets/icons/Server.webp';
import type {SourceInfo} from '@sd/ts-client';
import {getVolumeIcon} from '@sd/ts-client';

/**
 * The Spacedrive icon for a source, picked by the kind of drive under it.
 *
 * The kind comes from the source row, which keeps it while the drive is
 * away, so an unplugged disk still draws as one. A source on media
 * Spacedrive does not track falls back to the generic drive, and adapter
 * sources, which have no drive, share the database icon.
 */
export function sourceIcon(
	source: Pick<SourceInfo, 'data_type' | 'volume_type' | 'root'>
): string {
	if (source.data_type !== 'filesystem') return DatabaseIcon;
	switch (source.volume_type) {
		case 'External':
			return HDDIcon;
		case 'Network':
			return ServerIcon;
		case 'Virtual':
			return DatabaseIcon;
		case 'Cloud':
			return getVolumeIcon({mount_point: source.root});
		default:
			return DriveIcon;
	}
}
