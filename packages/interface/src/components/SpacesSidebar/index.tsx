import { useState, useEffect } from "react";
import { GearSix } from "@phosphor-icons/react";
import { useSidebarStore, useLibraryMutation } from "@sd/ts-client";
import type { SpaceGroup as SpaceGroupType, SpaceItem as SpaceItemType } from "@sd/ts-client";
import { useSpaces, useSpaceLayout } from "./hooks/useSpaces";
import { SpaceSwitcher } from "./SpaceSwitcher";
import { SpaceGroup } from "./SpaceGroup";
import { SpaceItem } from "./SpaceItem";
import { AddGroupButton } from "./AddGroupButton";
import { useSpacedriveClient } from "../../context";
import { useLibraries } from "../../hooks/useLibraries";
import { usePlatform } from "../../platform";
import { JobManagerPopover } from "../JobManager/JobManagerPopover";
import { SyncMonitorPopover } from "../SyncMonitor";
import clsx from "clsx";
import { useDroppable } from "@dnd-kit/core";
import { SortableContext, verticalListSortingStrategy, useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";

// Wrapper that adds a space-level drop zone before each group and makes it sortable
function SpaceGroupWithDropZone({
  group,
  items,
  spaceId,
  isFirst,
}: {
  group: SpaceGroupType;
  items: SpaceItemType[];
  spaceId?: string;
  isFirst: boolean;
}) {
  const { setNodeRef: setDropRef, isOver } = useDroppable({
    id: `space-root-before-${group.id}`,
    disabled: !spaceId,
    data: {
      action: 'add-to-space',
      spaceId,
      groupId: null,
    },
  });

  // Sortable for group reordering
  const {
    attributes,
    listeners,
    setNodeRef: setSortableRef,
    transform,
    transition,
    isDragging,
  } = useSortable({
    id: group.id,
  });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
  };

  return (
    <div ref={setSortableRef} style={style} className={clsx("relative", isDragging && "opacity-50 z-50")}>
      {/* Drop zone before this group (for adding root-level items) */}
      <div ref={setDropRef} className="absolute -top-2.5 left-0 right-0 h-5 z-10">
        {isOver && !isDragging && (
          <div className="absolute top-1/2 -translate-y-1/2 left-2 right-2 h-[2px] bg-accent rounded-full" />
        )}
      </div>
      <SpaceGroup
        group={group}
        items={items}
        spaceId={spaceId}
        sortableAttributes={attributes}
        sortableListeners={listeners}
      />
    </div>
  );
}

interface SpacesSidebarProps {
  isPreviewActive?: boolean;
}

export function SpacesSidebar({ isPreviewActive = false }: SpacesSidebarProps) {
  const client = useSpacedriveClient();
  const platform = usePlatform();
  const { data: libraries } = useLibraries();
  const [currentLibraryId, setCurrentLibraryId] = useState<string | null>(
    () => client.getCurrentLibraryId(),
  );

  const { currentSpaceId, setCurrentSpace } = useSidebarStore();
  const { data: spacesData } = useSpaces();
  const spaces = spacesData?.spaces;

  // Listen for library changes from client and update local state
  useEffect(() => {
    const handleLibraryChange = (newLibraryId: string) => {
      setCurrentLibraryId(newLibraryId);
    };

    client.on("library-changed", handleLibraryChange);
    return () => {
      client.off("library-changed", handleLibraryChange);
    };
  }, [client]);

  // Auto-select first library on mount if none selected
  useEffect(() => {
    if (libraries && libraries.length > 0 && !currentLibraryId) {
      const firstLib = libraries[0];

      // Set library ID via platform (syncs to all windows on Tauri)
      if (platform.setCurrentLibraryId) {
        platform.setCurrentLibraryId(firstLib.id).catch((err) =>
          console.error("Failed to set library ID:", err),
        );
      } else {
        // Web fallback - just update client
        client.setCurrentLibrary(firstLib.id);
      }
    }
  }, [libraries, currentLibraryId, client, platform]);

  // Auto-select first space if none selected
  const currentSpace =
    spaces?.find((s) => s.id === currentSpaceId) ?? spaces?.[0];

  useEffect(() => {
    if (currentSpace && currentSpace.id !== currentSpaceId) {
      setCurrentSpace(currentSpace.id);
    }
  }, [currentSpace, currentSpaceId, setCurrentSpace]);

  const { data: layout } = useSpaceLayout(currentSpace?.id ?? null);

  const addItem = useLibraryMutation("spaces.add_item");

  return (
    <div className="w-[220px] min-w-[176px] max-w-[300px] flex flex-col h-full p-2 bg-transparent">
      <div
        className={clsx(
          "flex flex-col h-full rounded-2xl overflow-hidden",
          isPreviewActive ? "backdrop-blur-2xl bg-sidebar/80" : "bg-sidebar/65",
        )}
      >
        <nav className="relative z-[51] flex h-full flex-col gap-2.5 p-2.5 pb-2 pt-[52px]">
          {/* Space Switcher */}
          <SpaceSwitcher
            spaces={spaces}
            currentSpace={currentSpace}
            onSwitch={setCurrentSpace}
          />

          {/* Scrollable Content */}
          <div className="no-scrollbar mt-3 mask-fade-out flex grow flex-col space-y-5 overflow-x-hidden overflow-y-scroll pb-10">
            {/* Space-level items (pinned shortcuts) */}
            {layout?.space_items && layout.space_items.length > 0 && (
              <SortableContext
                items={layout.space_items.map(item => item.id)}
                strategy={verticalListSortingStrategy}
              >
                <div className="space-y-0.5">
                  {layout.space_items.map((item, index) => (
                    <SpaceItem
                      key={item.id}
                      item={item}
                      isLastItem={index === layout.space_items.length - 1}
                      allowInsertion={true}
                      spaceId={currentSpace?.id}
                      groupId={null}
                      sortable={true}
                    />
                  ))}
                </div>
              </SortableContext>
            )}

            {/* Groups with space-level drop zones between them */}
            {layout?.groups && (
              <SortableContext
                items={layout.groups.map(({ group }) => group.id)}
                strategy={verticalListSortingStrategy}
              >
                {layout.groups.map(({ group, items }, index) => (
                  <SpaceGroupWithDropZone
                    key={group.id}
                    group={group}
                    items={items}
                    spaceId={currentSpace?.id}
                    isFirst={index === 0}
                  />
                ))}
              </SortableContext>
            )}

            {/* Add Group Button */}
            {currentSpace && <AddGroupButton spaceId={currentSpace.id} />}
          </div>

          {/* Sync Monitor, Job Manager & Settings (pinned to bottom) */}
          <div className="space-y-0.5">
            <SyncMonitorPopover />
            <JobManagerPopover />
            <button
              onClick={() => {
                if (platform.showWindow) {
                  platform.showWindow({ type: "Settings", page: "general" }).catch(err =>
                    console.error("Failed to open settings:", err)
                  );
                }
              }}
              className={clsx(
                "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm font-medium transition-colors",
                "text-sidebar-inkDull hover:text-sidebar-ink hover:bg-sidebar-selected",
              )}
            >
              <GearSix className="size-4" weight="bold" />
              <span className="truncate">Settings</span>
            </button>
          </div>
        </nav>
      </div>
    </div>
  );
}
