import { CaretRight } from '@phosphor-icons/react';
import clsx from 'clsx';
import { useState, useEffect, useRef } from 'react';
import type {
	SpaceGroup as SpaceGroupType,
	SpaceItem as SpaceItemType,
} from '@sd/ts-client';
import { useSidebarStore, useLibraryMutation } from '@sd/ts-client';
import { SpaceItem } from './SpaceItem';
import { DeviceGroup } from './DeviceGroup';
import { DevicesGroup } from './DevicesGroup';
import { LocationsGroup } from './LocationsGroup';
import { VolumesGroup } from './VolumesGroup';
import { TagsGroup } from './TagsGroup';
import { getDragData, clearDragData, subscribeToDragState } from './dnd';
import { usePlatform } from '../../platform';

interface SpaceGroupProps {
	group: SpaceGroupType;
	items: SpaceItemType[];
	spaceId?: string;
}

export function SpaceGroup({ group, items, spaceId }: SpaceGroupProps) {
	const { collapsedGroups, toggleGroup } = useSidebarStore();
	const platform = usePlatform();
	// Use backend's is_collapsed value as the source of truth, fallback to local state
	const isCollapsed = group.is_collapsed ?? collapsedGroups.has(group.id);

	// Drag-drop state for custom groups
	const [isDragging, setIsDragging] = useState(false);
	const [isHovering, setIsHovering] = useState(false);
	const addItem = useLibraryMutation("spaces.add_item");
	const groupRef = useRef<HTMLDivElement>(null);

	// Only QuickAccess and Custom groups can accept drops
	const canAcceptDrop = group.group_type === 'QuickAccess' || group.group_type === 'Custom';

	// Subscribe to drag state changes
	useEffect(() => {
		if (!canAcceptDrop) return;
		return subscribeToDragState(setIsDragging);
	}, [canAcceptDrop]);

	// Listen for native drag events to track position and handle drop
	useEffect(() => {
		if (!platform.onDragEvent || !canAcceptDrop) return;

		const unlisteners: Array<() => void> = [];

		// Track drag position to detect when over this group
		platform.onDragEvent("moved", (payload: { x: number; y: number }) => {
			if (!groupRef.current) return;

			const rect = groupRef.current.getBoundingClientRect();
			const isOver = (
				payload.x >= rect.left &&
				payload.x <= rect.right &&
				payload.y >= rect.top &&
				payload.y <= rect.bottom
			);
			setIsHovering(isOver);
		}).then(fn => unlisteners.push(fn));

		// Handle drag end - check if dropped on this group
		platform.onDragEvent("ended", async (payload: { result?: { type: string } }) => {
			if (payload.result?.type === "Dropped" && isHovering && spaceId) {
				const dragData = getDragData();
				if (dragData) {
					try {
						await addItem.mutateAsync({
							space_id: spaceId,
							group_id: group.id,
							item_type: { Path: { sd_path: dragData.sdPath } },
						});
						console.log("[SpaceGroup] Added item to group:", group.name);
					} catch (err) {
						console.error("Failed to add item to group:", err);
					}
				}
			}
			setIsDragging(false);
			setIsHovering(false);
		}).then(fn => unlisteners.push(fn));

		return () => {
			unlisteners.forEach(fn => fn());
		};
	}, [platform, canAcceptDrop, spaceId, group.id, group.name, addItem, isHovering]);

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
			ref={groupRef}
			className={clsx(
				"rounded-lg transition-colors",
				isDragging && canAcceptDrop && "bg-accent/10 ring-2 ring-accent/50 ring-inset",
				isDragging && isHovering && canAcceptDrop && "bg-accent/20 ring-accent"
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
					{/* Drop hint */}
					{isDragging && canAcceptDrop && (
						<div className={clsx(
							"flex items-center justify-center py-2 text-xs font-medium transition-colors",
							isHovering ? "text-accent" : "text-accent/70"
						)}>
							{isHovering ? "Release to add" : "Drop here"}
						</div>
					)}
				</div>
			)}
		</div>
	);
}
