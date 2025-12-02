import { CaretRight } from '@phosphor-icons/react';
import clsx from 'clsx';
import { useState, useEffect } from 'react';
import type {
	SpaceGroup as SpaceGroupType,
	SpaceItem as SpaceItemType,
} from '@sd/ts-client';
import { useSidebarStore } from '@sd/ts-client';
import { SpaceItem } from './SpaceItem';
import { DeviceGroup } from './DeviceGroup';
import { DevicesGroup } from './DevicesGroup';
import { LocationsGroup } from './LocationsGroup';
import { VolumesGroup } from './VolumesGroup';
import { TagsGroup } from './TagsGroup';
import { subscribeToDragState } from './dnd';

interface SpaceGroupProps {
	group: SpaceGroupType;
	items: SpaceItemType[];
	spaceId?: string;
}

export function SpaceGroup({ group, items, spaceId }: SpaceGroupProps) {
	const { collapsedGroups, toggleGroup } = useSidebarStore();
	// Use backend's is_collapsed value as the source of truth, fallback to local state
	const isCollapsed = group.is_collapsed ?? collapsedGroups.has(group.id);

	// Drag-drop state for custom groups
	const [isDragging, setIsDragging] = useState(false);

	// Only QuickAccess and Custom groups can accept drops
	const canAcceptDrop = group.group_type === 'QuickAccess' || group.group_type === 'Custom';

	// Subscribe to drag state changes
	useEffect(() => {
		if (!canAcceptDrop) return;
		return subscribeToDragState(setIsDragging);
	}, [canAcceptDrop]);

	// TODO: Drop handling for groups - need to coordinate with parent to avoid duplicates

	// Device groups are special - they show device info with children
	if (typeof group.group_type === 'object' && 'Device' in group.group_type) {
		return (
			<DeviceGroup
				deviceId={group.group_type.Device.device_id}
				items={items}
				isCollapsed={isCollapsed}
				onToggle={() => toggleGroup(group.id)}
			/>
		);
	}

	// Devices group - fetches all devices (library + paired)
	if (group.group_type === 'Devices') {
		return <DevicesGroup isCollapsed={isCollapsed} onToggle={() => toggleGroup(group.id)} />;
	}

	// Locations group - fetches all locations
	if (group.group_type === 'Locations') {
		return <LocationsGroup isCollapsed={isCollapsed} onToggle={() => toggleGroup(group.id)} />;
	}

	// Volumes group - fetches all volumes
	if (group.group_type === 'Volumes') {
		return <VolumesGroup isCollapsed={isCollapsed} onToggle={() => toggleGroup(group.id)} />;
	}

	// Tags group - fetches all tags
	if (group.group_type === 'Tags') {
		return <TagsGroup isCollapsed={isCollapsed} onToggle={() => toggleGroup(group.id)} />;
	}

	// QuickAccess and Custom groups render stored items
	return (
		<div
			className={clsx(
				"rounded-lg transition-colors"
				// TODO: Add drop zone styling when group-level drops are implemented
			)}
		>
			{/* Group Header */}
			<button
				onClick={() => toggleGroup(group.id)}
				className="mb-1 flex w-full items-center gap-2 px-1 text-xs font-semibold uppercase tracking-wider text-sidebar-ink-faint hover:text-sidebar-ink"
			>
				<CaretRight
					className={clsx('transition-transform', !isCollapsed && 'rotate-90')}
					size={10}
					weight="bold"
				/>
				<span>{group.name}</span>
			</button>

			{/* Items */}
			{!isCollapsed && (
				<div className="space-y-0.5">
					{items.map((item) => (
						<SpaceItem key={item.id} item={item} />
					))}
				</div>
			)}
		</div>
	);
}
