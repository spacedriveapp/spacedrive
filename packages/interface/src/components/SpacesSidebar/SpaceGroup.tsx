import type {
  SpaceGroup as SpaceGroupType,
  SpaceItem as SpaceItemType,
} from "@sd/ts-client";
import { useSidebarStore } from "@sd/ts-client";
import { SpaceItem } from "./SpaceItem";
import { DeviceGroup } from "./DeviceGroup";
import { DevicesGroup } from "./DevicesGroup";
import { LocationsGroup } from "./LocationsGroup";
import { VolumesGroup } from "./VolumesGroup";
import { TagsGroup } from "./TagsGroup";
import { GroupHeader } from "./GroupHeader";
import { useDroppable } from "@dnd-kit/core";

interface SpaceGroupProps {
  group: SpaceGroupType;
  items: SpaceItemType[];
  spaceId?: string;
  sortableAttributes?: any;
  sortableListeners?: any;
}

export function SpaceGroup({ group, items, spaceId, sortableAttributes, sortableListeners }: SpaceGroupProps) {
  const { collapsedGroups, toggleGroup } = useSidebarStore();
  // Use backend's is_collapsed value as the source of truth, fallback to local state
  const isCollapsed = group.is_collapsed ?? collapsedGroups.has(group.id);

  // System groups (Locations, Volumes, etc.) are dynamic - don't allow insertion/reordering
  // Custom/QuickAccess groups allow insertion
  const allowInsertion =
    group.group_type === "QuickAccess" || group.group_type === "Custom";

  // Device groups are special - they show device info with children
  if (typeof group.group_type === "object" && "Device" in group.group_type) {
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
  if (group.group_type === "Devices") {
    return (
      <DevicesGroup
        isCollapsed={isCollapsed}
        onToggle={() => toggleGroup(group.id)}
      />
    );
  }

  // Locations group - fetches all locations
  if (group.group_type === "Locations") {
    return (
      <LocationsGroup
        isCollapsed={isCollapsed}
        onToggle={() => toggleGroup(group.id)}
      />
    );
  }

  // Volumes group - fetches all volumes
  if (group.group_type === "Volumes") {
    return (
      <VolumesGroup
        isCollapsed={isCollapsed}
        onToggle={() => toggleGroup(group.id)}
      />
    );
  }

  // Tags group - fetches all tags
  if (group.group_type === "Tags") {
    return (
      <TagsGroup
        isCollapsed={isCollapsed}
        onToggle={() => toggleGroup(group.id)}
      />
    );
  }

  // Empty drop zone for groups with no items
  const { setNodeRef: setEmptyRef, isOver: isOverEmpty } = useDroppable({
    id: `group-${group.id}-empty`,
    disabled: !allowInsertion || isCollapsed,
    data: {
      action: "add-to-group",
      groupId: group.id,
      spaceId,
    },
  });

  // QuickAccess and Custom groups render stored items
  return (
    <div className="rounded-lg">
      <GroupHeader
        label={group.name}
        isCollapsed={isCollapsed}
        onToggle={() => toggleGroup(group.id)}
        sortableAttributes={sortableAttributes}
        sortableListeners={sortableListeners}
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
            <div ref={setEmptyRef} className="absolute inset-0 z-10">
              {isOverEmpty && (
                <div className="absolute top-1/2 -translate-y-1/2 left-2 right-2 h-[2px] bg-accent rounded-full" />
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
