import {EyeSlash, Plugs, WifiSlash} from '@phosphor-icons/react';
import {getVolumeIcon, useNormalizedQuery} from '@sd/ts-client';
import type {Volume} from '@sd/ts-client';
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
function VolumeItem({volume, index, volumesLength}: {volume: Volume; index: number; volumesLength: number}) {
	const contextMenu = useVolumeContextMenu({volume});

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
			volumeData={{
				device_slug: volume.device_id,
				mount_path: volume.mount_point || '/'
			}}
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
		wireMethod: 'query:volumes.list',
		input: {filter},
		resourceType: 'volume'
	});

	const volumes = volumesData?.volumes || [];

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
							/>
						))
					)}
				</div>
			)}
		</div>
	);
}
