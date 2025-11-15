import { CaretRight, Desktop } from '@phosphor-icons/react';
import clsx from 'clsx';
import type { SpaceItem as SpaceItemType } from '@sd/ts-client/generated/types';
import { SpaceItem } from './SpaceItem';

interface DeviceGroupProps {
	deviceId: string;
	items: SpaceItemType[];
	isCollapsed: boolean;
	onToggle: () => void;
}

export function DeviceGroup({ deviceId, items, isCollapsed, onToggle }: DeviceGroupProps) {
	// TODO: Fetch actual device data
	const deviceName = 'Device'; // Placeholder

	return (
		<div>
			{/* Device Header */}
			<button
				onClick={onToggle}
				className="flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-sm hover:bg-sidebar-selected/40"
			>
				<CaretRight
					className={clsx('transition-transform', !isCollapsed && 'rotate-90')}
					size={12}
					weight="bold"
				/>
				<Desktop size={16} weight="bold" className="text-sidebar-ink-dull" />
				<span className="flex-1 truncate text-sidebar-ink">{deviceName}</span>
				{/* TODO: Add online indicator */}
			</button>

			{/* Children (Volumes & Locations from items) */}
			{!isCollapsed && (
				<div className="ml-4 mt-1 space-y-0.5">
					{items.map((item) => (
						<SpaceItem key={item.id} item={item} />
					))}
				</div>
			)}
		</div>
	);
}
