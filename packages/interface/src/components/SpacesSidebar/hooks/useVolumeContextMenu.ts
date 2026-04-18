import {
	Database,
	EyeSlash,
	Eye,
	Gauge,
	EjectSimple,
	PlugsConnected
} from '@phosphor-icons/react';
import type { Volume } from '@sd/ts-client';
import {
	useContextMenu,
	type ContextMenuItem,
	type ContextMenuResult
} from '../../../hooks/useContextMenu';
import { useLibraryMutation } from '../../../contexts/SpacedriveContext';
import { useDisconnectCloudVolumeDialog } from '../DisconnectCloudVolumeDialog';

interface UseVolumeContextMenuOptions {
	volume: Volume;
}

/** Context menu for a volume row. Cloud volumes hide Speed Test and Eject
 *  (no meaningful semantics), and expose Disconnect which purges credentials
 *  via `volumes.remove_cloud` — distinct from the generic Untrack which only
 *  removes the volume from the library. */
export function useVolumeContextMenu({
	volume
}: UseVolumeContextMenuOptions): ContextMenuResult {
	const trackVolume = useLibraryMutation('volumes.track');
	const untrackVolume = useLibraryMutation('volumes.untrack');
	const speedTestVolume = useLibraryMutation('volumes.speed_test');
	const indexVolume = useLibraryMutation('volumes.index');
	const ejectVolume = useLibraryMutation('volumes.eject');
	const openDisconnectDialog = useDisconnectCloudVolumeDialog();

	const isRemovable = volume.mount_type === 'External';
	const isCloud = volume.volume_type === 'Cloud';

	const items: ContextMenuItem[] = [
		{
			icon: Eye,
			label: 'Track Volume',
			onClick: async () => {
				try {
					await trackVolume.mutateAsync({
						fingerprint: volume.fingerprint,
						display_name: null
					});
				} catch (err) {
					console.error('Failed to track volume:', err);
				}
			},
			condition: () => !volume.is_tracked
		},
		{
			icon: EyeSlash,
			label: 'Untrack Volume',
			onClick: async () => {
				try {
					await untrackVolume.mutateAsync({
						volume_id: volume.id
					});
				} catch (err) {
					console.error('Failed to untrack volume:', err);
				}
			},
			variant: 'danger' as const,
			condition: () => volume.is_tracked
		},
		{
			icon: PlugsConnected,
			label: 'Disconnect',
			onClick: () => {
				openDisconnectDialog(volume.fingerprint, volume.name);
			},
			variant: 'danger' as const,
			condition: () => isCloud
		},
		{ type: 'separator' },
		{
			icon: Database,
			label: 'Index Volume',
			onClick: async () => {
				try {
					const result = await indexVolume.mutateAsync({
						fingerprint: volume.fingerprint,
						scope: 'Recursive'
					});
					console.log('Volume indexed:', result.message);
				} catch (err) {
					console.error('Failed to index volume:', err);
				}
			},
			condition: () => volume.is_mounted
		},
		{
			icon: Gauge,
			label: 'Speed Test',
			onClick: async () => {
				try {
					const result = await speedTestVolume.mutateAsync({
						fingerprint: volume.fingerprint
					});
					console.log(
						'Speed test complete:',
						result.read_speed_mbps,
						'MB/s read,',
						result.write_speed_mbps,
						'MB/s write'
					);
				} catch (err) {
					console.error('Failed to run speed test:', err);
				}
			},
			condition: () => !isCloud && volume.is_mounted
		},
		{
			icon: EjectSimple,
			label: 'Eject',
			onClick: async () => {
				try {
					const result = await ejectVolume.mutateAsync({
						fingerprint: volume.fingerprint
					});
					if (result.success) {
						console.log('Volume ejected successfully');
					} else {
						console.error('Eject failed:', result.message);
					}
				} catch (err) {
					console.error('Failed to eject volume:', err);
				}
			},
			keybind: '⌘E',
			condition: () => !isCloud && isRemovable && volume.is_mounted
		}
	];

	return useContextMenu({ items });
}
