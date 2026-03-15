import {EyeSlash, Plugs, WifiSlash} from '@phosphor-icons/react';
import {getVolumeIcon, useNormalizedQuery} from '@sd/ts-client';
import type {Device, Volume} from '@sd/ts-client';
import {useNavigate} from 'react-router-dom';
import {GroupHeader} from './GroupHeader';
import {SpaceItem} from './SpaceItem';
import {useVolumeContextMenu} from './hooks/useVolumeContextMenu';

interface VolumesGroupProps {
	isCollapsed: boolean;
	onToggle: () => void;
	/** Filter to show tracked, untracked, or all volumes (default: "All") */
	filter?: 'TrackedOnly' | 'UntrackedOnly' | 'All';
	sortableAttributes?: any;
	sortableListeners?: any;
}

// Helper to render volume status indicator
const getVolumeIndicator = (volume: Volume) => (
	<>
		{!volume.is_tracked && (
			<EyeSlash
				size={14}
				weight="bold"
				className="text-ink-faint/50"
			/>
		)}
	</>
);

// Component for individual volume items with context menu
function VolumeItem({volume, index, volumesLength, devices}: {volume: Volume; index: number; volumesLength: number; devices: Device[]}) {
	const contextMenu = useVolumeContextMenu({volume});

	// Look up the device by ID to get the slug (not the UUID)
	const device = devices.find((d) => d.id === volume.device_id);
	const deviceSlug = device?.slug;

	return (
		<SpaceItem
			key={volume.id}
			item={
				{
					id: volume.id,
					item_type: {
						Volume: {
							volume_id: volume.id,
							name: volume.display_name || volume.name
						}
					}
				} as any
			}
			volumeData={deviceSlug ? {
				device_slug: deviceSlug,
				mount_path: volume.mount_point || '/'
			} : undefined}
			rightComponent={getVolumeIndicator(volume)}
			customIcon={getVolumeIcon(volume)}
			allowInsertion={false}
			isLastItem={index === volumesLength - 1}
			onContextMenu={contextMenu.show}
		/>
	);
}

export function VolumesGroup({
	isCollapsed,
	onToggle,
	filter = 'All',
	sortableAttributes,
	sortableListeners
}: VolumesGroupProps) {
	const {data: volumesData} = useNormalizedQuery({
		query: 'volumes.list',
		input: {filter},
		resourceType: 'volume'
	});

	const {data: devicesData} = useNormalizedQuery({
		query: 'devices.list',
		input: {include_offline: true, include_details: false},
		resourceType: 'device'
	});

	const volumes = volumesData?.volumes || [];
	const devices: Device[] = (devicesData as Device[]) ?? [];

	return (
		<div>
			<GroupHeader
				label="Volumes"
				isCollapsed={isCollapsed}
				onToggle={onToggle}
				sortableAttributes={sortableAttributes}
				sortableListeners={sortableListeners}
			/>

			{/* Volumes List */}
			{!isCollapsed && (
				<div className="space-y-0.5">
					{volumes.length === 0 ? (
						<div className="text-ink-faint px-2 py-1 text-xs">
							No volumes
						</div>
					) : (
						volumes.map((volume, index) => (
							<VolumeItem
								key={volume.id}
								volume={volume}
								index={index}
								volumesLength={volumes.length}
								devices={devices}
							/>
						))
					)}
				</div>
			)}
		</div>
	);
}
