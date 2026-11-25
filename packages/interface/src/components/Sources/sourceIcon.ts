import DatabaseLightIcon from '@sd/assets/icons/Database_Light.webp';
import DatabaseIcon from '@sd/assets/icons/Database.webp';
import DriveLightIcon from '@sd/assets/icons/Drive_Light.webp';
import DriveIcon from '@sd/assets/icons/Drive.webp';
import HDDLightIcon from '@sd/assets/icons/HDD_Light.webp';
import HDDIcon from '@sd/assets/icons/HDD.webp';
import ServerLightIcon from '@sd/assets/icons/Server_Light.webp';
import ServerIcon from '@sd/assets/icons/Server.webp';
import type {SourceInfo} from '@sd/ts-client';
import {getVolumeIcon} from '@sd/ts-client';

/**
 * The Spacedrive icon for a source, picked by the kind of drive under it.
 *
 * The kind comes from the source row, which keeps it while the drive is
 * away, so an unplugged disk still draws as one. A replica lives on a
 * paired device, such as a NAS, and draws as a server whatever its owner's
 * drive is. A source on media Spacedrive does not track falls back to the
 * generic drive, and adapter sources, which have no drive, share the
 * database icon.
 *
 * The plain assets are dark and vanish on a dark sidebar, so dark themes
 * get the pale `_Light` variants and the light theme gets the dark ones.
 */
export function sourceIcon(
	source: Pick<
		SourceInfo,
		'data_type' | 'volume_type' | 'root' | 'device_id'
	>,
	lightTheme: boolean
): string {
	const pick = (dark: string, pale: string) => (lightTheme ? dark : pale);
	if (source.data_type !== 'filesystem')
		return pick(DatabaseIcon, DatabaseLightIcon);
	if (source.device_id) return pick(ServerIcon, ServerLightIcon);
	switch (source.volume_type) {
		case 'External':
			return pick(HDDIcon, HDDLightIcon);
		case 'Network':
			return pick(ServerIcon, ServerLightIcon);
		case 'Virtual':
			return pick(DatabaseIcon, DatabaseLightIcon);
		case 'Cloud':
			return getVolumeIcon({mount_point: source.root});
		default:
			return pick(DriveIcon, DriveLightIcon);
	}
}
