import type {
	SpaceGroup as SpaceGroupType,
	SpaceItem as SpaceItemType,
} from "@sd/ts-client";
import { useSidebarStore, useLibraryMutation } from "@sd/ts-client";
import { SpaceItem } from "./SpaceItem";
import { DevicesGroup } from "./DevicesGroup";
import { LocationsGroup } from "./LocationsGroup";
import { VolumesGroup } from "./VolumesGroup";
import { TagsGroup } from "./TagsGroup";
import { GroupHeader } from "./GroupHeader";
import { useDroppable, useDndContext } from "@dnd-kit/core";

interface SpaceGroupProps {
	group: SpaceGroupType;
	items: SpaceItemType[];
	spaceId?: string;
	sortableAttributes?: any;
	sortableListeners?: any;
}

export function SpaceGroup({
	group,
	items,
	spaceId,
	sortableAttributes,
	sortableListeners,
}: SpaceGroupProps) {
	const { collapsedGroups, toggleGroup: toggleGroupLocal } = useSidebarStore();
	const { active } = useDndContext();
	const updateGroup = useLibraryMutation("spaces.update_group");
	
	// Use backend's is_collapsed value as the source of truth, fallback to local state
	const isCollapsed = group.is_collapsed ?? collapsedGroups.has(group.id);
	
	// Toggle handler that updates both local and backend state
	const handleToggle = async () => {
		// Optimistically update local state for immediate UI feedback
		toggleGroupLocal(group.id);
		
		// Update backend
		try {
			await updateGroup.mutateAsync({
				group_id: group.id,
				is_collapsed: !isCollapsed,
			});
		} catch (error) {
			console.error("Failed to update group collapse state:", error);
			// Revert local state on error
			toggleGroupLocal(group.id);
		}
	};

	// Disable insertion drop zones when dragging groups or space items (they have 'label' in their data)
	const isDraggingSortableItem = active?.data?.current?.label != null;

	// System groups (Locations, Volumes, etc.) are dynamic - don't allow insertion/reordering
	// Custom/QuickAccess groups allow insertion
	const allowInsertion =
		group.group_type === "QuickAccess" || group.group_type === "Custom";

	// Devices group - fetches all devices (library + paired)
	if (group.group_type === "Devices") {
		return (
			<div data-group-id={group.id}>
				<DevicesGroup
					isCollapsed={isCollapsed}
					onToggle={handleToggle}
					sortableAttributes={sortableAttributes}
					sortableListeners={sortableListeners}
				/>
			</div>
		);
	}

	// Locations group - fetches all locations
	if (group.group_type === "Locations") {
		return (
			<div data-group-id={group.id}>
				<LocationsGroup
					isCollapsed={isCollapsed}
					onToggle={handleToggle}
					sortableAttributes={sortableAttributes}
					sortableListeners={sortableListeners}
				/>
			</div>
		);
	}

	// Volumes group - fetches all volumes
	if (group.group_type === "Volumes") {
		return (
			<div data-group-id={group.id}>
				<VolumesGroup
					isCollapsed={isCollapsed}
					onToggle={handleToggle}
					sortableAttributes={sortableAttributes}
					sortableListeners={sortableListeners}
				/>
			</div>
		);
	}

	// Tags group - fetches all tags
	if (group.group_type === "Tags") {
		return (
			<div data-group-id={group.id}>
				<TagsGroup
					isCollapsed={isCollapsed}
					onToggle={handleToggle}
					sortableAttributes={sortableAttributes}
					sortableListeners={sortableListeners}
				/>
			</div>
		);
	}

	// Empty drop zone for groups with no items
	const { setNodeRef: setEmptyRef, isOver: isOverEmpty } = useDroppable({
		id: `group-${group.id}-empty`,
		disabled: !allowInsertion || isCollapsed || isDraggingSortableItem,
		data: {
			action: "add-to-group",
			groupId: group.id,
			spaceId,
		},
	});

	// QuickAccess and Custom groups render stored items
	return (
		<div className="rounded-lg" data-group-id={group.id}>
			<GroupHeader
				label={group.name}
				isCollapsed={isCollapsed}
				onToggle={handleToggle}
				sortableAttributes={sortableAttributes}
				sortableListeners={sortableListeners}
				group={group}
				allowCustomization={allowInsertion}
			/>

			{/* Items */}
			{!isCollapsed && (
				<div className="space-y-0.5 relative min-h-[20px]">
					{items.length > 0 ? (
						items.map((item, index) => (
							<SpaceItem
								key={item.id}
								item={item}
								isLastItem={index === items.length - 1}
								allowInsertion={allowInsertion}
								spaceId={spaceId}
								groupId={group.id}
							/>
						))
					) : (
						<div
							ref={setEmptyRef}
							className="absolute inset-0 z-10"
						>
					{isOverEmpty && !isDraggingSortableItem && (
						<div className="absolute top-1/2 -translate-y-1/2 left-2 right-2 h-[2px] bg-accent rounded-full" />
					)}
						</div>
					)}
				</div>
			)}
		</div>
	);
}
