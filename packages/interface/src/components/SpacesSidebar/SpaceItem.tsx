import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import type { SpaceItem as SpaceItemType } from "@sd/ts-client";
import clsx from "clsx";
import { useNavigate } from "react-router-dom";
import {
  getSpaceItemKeyFromRoute,
  useExplorer,
} from "../../routes/explorer/context";
import { Thumb } from "../../routes/explorer/File/Thumb";
import {
  type IconData,
  isRawLocation,
  resolveItemMetadata,
} from "./hooks/spaceItemUtils";
import { useSpaceItemActive } from "./hooks/useSpaceItemActive";
import { useSpaceItemContextMenu } from "./hooks/useSpaceItemContextMenu";
import { useSpaceItemDropZones } from "./hooks/useSpaceItemDropZones";

// Overrides for customizing item appearance and behavior
export interface SpaceItemOverrides {
  label?: string;
  icon?: string;
  onClick?: (e?: React.MouseEvent) => void;
  onContextMenu?: (e: React.MouseEvent) => void;
}

export interface SpaceItemProps {
  item: SpaceItemType;
  spaceId?: string;
  groupId?: string | null;
  // Behavior flags
  sortable?: boolean;
  allowInsertion?: boolean;
  isLastItem?: boolean;
  // Overrides
  overrides?: SpaceItemOverrides;
  rightComponent?: React.ReactNode;
  // Legacy props (for backwards compatibility during migration)
  volumeData?: { device_slug: string; mount_path: string };
  customIcon?: string;
  customLabel?: string;
  onClick?: (e?: React.MouseEvent) => void;
  onContextMenu?: (e: React.MouseEvent) => void;
  className?: string;
}

// Icon component that handles both component icons and image icons
function ItemIcon({ icon }: { icon: IconData }) {
  if (icon.type === "image") {
    return (
      <img
        alt=""
        className="size-4 shrink-0"
        height={16}
        src={icon.icon}
        width={16}
      />
    );
  }
  const IconComponent = icon.icon;
  return (
    <span className="shrink-0">
      <IconComponent size={16} weight="bold" />
    </span>
  );
}

// Insertion line indicator
function InsertionLine({ visible }: { visible: boolean }) {
  if (!visible) {
    return null;
  }
  return (
    <div className="absolute -top-[1px] right-2 left-2 z-20 h-[2px] rounded-full bg-accent" />
  );
}

// Bottom insertion line (for last items)
function BottomInsertionLine({ visible }: { visible: boolean }) {
  if (!visible) {
    return null;
  }
  return (
    <div className="absolute right-2 -bottom-[1px] left-2 z-20 h-[2px] rounded-full bg-accent" />
  );
}

// Drop highlight ring for drop-into targets
function DropHighlight({ visible }: { visible: boolean }) {
  if (!visible) {
    return null;
  }
  return (
    <div className="pointer-events-none absolute inset-0 z-10 rounded-md ring-2 ring-accent/50 ring-inset" />
  );
}

// Drop zone overlays (invisible hit areas)
interface DropZoneOverlaysProps {
  isDropTarget: boolean;
  setTopRef: (node: HTMLElement | null) => void;
  setBottomRef: (node: HTMLElement | null) => void;
  setMiddleRef: (node: HTMLElement | null) => void;
}

function DropZoneOverlays({
  isDropTarget,
  setTopRef,
  setBottomRef,
  setMiddleRef,
}: DropZoneOverlaysProps) {
  if (isDropTarget) {
    return (
      <>
        {/* Top zone - insertion above */}
        <div
          className="pointer-events-none absolute right-0 left-0"
          ref={setTopRef}
          style={{
            top: "-2px",
            height: "calc(25% + 2px)",
            zIndex: 10,
          }}
        />
        {/* Middle zone - drop into folder */}
        <div
          className="pointer-events-none absolute right-0 left-0"
          ref={setMiddleRef}
          style={{ top: "25%", height: "50%", zIndex: 11 }}
        />
        {/* Bottom zone - insertion below */}
        <div
          className="pointer-events-none absolute right-0 left-0"
          ref={setBottomRef}
          style={{
            bottom: "-2px",
            height: "calc(25% + 2px)",
            zIndex: 10,
          }}
        />
      </>
    );
  }

  return (
    <>
      {/* Top zone - insertion above */}
      <div
        className="pointer-events-none absolute right-0 left-0"
        ref={setTopRef}
        style={{ top: "-2px", height: "calc(50% + 2px)", zIndex: 10 }}
      />
      {/* Bottom zone - insertion below */}
      <div
        className="pointer-events-none absolute right-0 left-0"
        ref={setBottomRef}
        style={{
          bottom: "-2px",
          height: "calc(50% + 2px)",
          zIndex: 10,
        }}
      />
    </>
  );
}

export function SpaceItem({
  item,
  spaceId,
  groupId,
  sortable = false,
  allowInsertion = true,
  isLastItem = false,
  overrides,
  rightComponent,
  // Legacy props
  volumeData,
  customIcon,
  customLabel,
  onClick: legacyOnClick,
  onContextMenu: legacyOnContextMenu,
  className,
}: SpaceItemProps) {
  const navigate = useNavigate();
  const { loadPreferencesForSpaceItem } = useExplorer();

  // Merge legacy props into overrides
  const effectiveOverrides: SpaceItemOverrides = {
    ...overrides,
    label: overrides?.label ?? customLabel,
    icon: overrides?.icon ?? customIcon,
    onClick: overrides?.onClick ?? legacyOnClick,
    onContextMenu: overrides?.onContextMenu ?? legacyOnContextMenu,
  };

  // Resolve metadata (icon, label, path)
  const { icon, label, path } = resolveItemMetadata(item, {
    volumeData,
    customIcon: effectiveOverrides.icon,
    customLabel: effectiveOverrides.label,
  });

  // Get resolved file for thumbnail rendering
  const resolvedFile = isRawLocation(item)
    ? undefined
    : (item as SpaceItemType).resolved_file;

  // Active state detection
  const isActive = useSpaceItemActive({
    item: item as SpaceItemType,
    path,
    hasCustomOnClick: !!effectiveOverrides.onClick,
  });

  // Drop zone management
  const dropZones = useSpaceItemDropZones({
    item: item as SpaceItemType,
    allowInsertion,
    spaceId,
    groupId,
    volumeData,
  });

  // Context menu
  const contextMenu = useSpaceItemContextMenu({
    item: item as SpaceItemType,
    path,
    spaceId,
  });

  // Sortable drag/drop
  const {
    attributes: sortableAttributes,
    listeners: sortableListeners,
    setNodeRef: setSortableRef,
    transform,
    transition,
    isDragging: isSortableDragging,
  } = useSortable({
    id: (item as SpaceItemType).id,
    disabled: !sortable,
    data: { label },
  });

  const style = sortable
    ? {
        transform: CSS.Transform.toString(transform),
        transition,
      }
    : undefined;

  // Event handlers
  const handleClick = (e: React.MouseEvent) => {
    if (effectiveOverrides.onClick) {
      effectiveOverrides.onClick(e);
    } else if (path) {
      // Extract pathname and search from the path
      const [pathname, search] = path.includes("?")
        ? [path.split("?")[0], `?${path.split("?")[1]}`]
        : [path, ""];
      const spaceItemKey = getSpaceItemKeyFromRoute(pathname, search);
      loadPreferencesForSpaceItem(spaceItemKey);
      navigate(path);
    }
  };

  const handleContextMenu = async (e: React.MouseEvent) => {
    if (effectiveOverrides.onContextMenu) {
      effectiveOverrides.onContextMenu(e);
      return;
    }

    e.preventDefault();
    e.stopPropagation();
    await contextMenu.show(e);
  };

  // Computed visibility for indicators
  const showTopLine =
    dropZones.isOverTop &&
    !isSortableDragging &&
    !dropZones.isDraggingSortableItem;
  const showBottomLine =
    dropZones.isOverBottom && isLastItem && !dropZones.isDraggingSortableItem;
  const showDropHighlight =
    dropZones.isOverMiddle &&
    dropZones.isDropTarget &&
    !isSortableDragging &&
    !dropZones.isDraggingSortableItem;

  return (
    <div
      className={clsx("relative", isSortableDragging && "z-50 opacity-50")}
      ref={setSortableRef}
      style={style}
    >
      <InsertionLine visible={showTopLine} />
      <DropHighlight visible={showDropHighlight} />

      <div className="relative">
        <DropZoneOverlays
          isDropTarget={dropZones.isDropTarget}
          setBottomRef={dropZones.setBottomRef}
          setMiddleRef={dropZones.setMiddleRef}
          setTopRef={dropZones.setTopRef}
        />

        <button
          onClick={handleClick}
          onContextMenu={handleContextMenu}
          {...(sortable ? { ...sortableAttributes, ...sortableListeners } : {})}
          className={clsx(
            "relative flex w-full cursor-default items-center gap-2 rounded-md px-2 py-1.5 font-medium text-sm transition-colors",
            isActive
              ? "bg-sidebar-selected/30 text-sidebar-ink"
              : className || "text-sidebar-inkDull",
            showDropHighlight && "bg-accent/10"
          )}
        >
          {resolvedFile ? (
            <Thumb className="shrink-0" file={resolvedFile} size={16} />
          ) : (
            <ItemIcon icon={icon} />
          )}
          <span className="flex-1 truncate text-left">{label}</span>
          {rightComponent}
        </button>
      </div>

      <BottomInsertionLine visible={showBottomLine} />
    </div>
  );
}
