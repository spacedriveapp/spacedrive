import { useDndContext, useDroppable } from "@dnd-kit/core";
import {
  SortableContext,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import {
  ArrowsClockwise,
  ArrowsOut,
  CircleNotch,
  FunnelSimple,
  GearSix,
  ListBullets,
  Palette,
} from "@phosphor-icons/react";
import type {
  SpaceGroup as SpaceGroupType,
  SpaceItem as SpaceItemType,
} from "@sd/ts-client";
import { useLibraryMutation, useSidebarStore } from "@sd/ts-client";
import { Popover, TopBarButton, usePopover } from "@sd/ui";
import clsx from "clsx";
import { motion } from "framer-motion";
import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { usePlatform } from "../../contexts/PlatformContext";
import { useSpacedriveClient } from "../../contexts/SpacedriveContext";
import { useLibraries } from "../../hooks/useLibraries";
import { JobList } from "../JobManager/components/JobList";
import { useJobs } from "../JobManager/hooks/useJobs";
import { CARD_HEIGHT } from "../JobManager/types";
import { ActivityFeed } from "../SyncMonitor/components/ActivityFeed";
import { PeerList } from "../SyncMonitor/components/PeerList";
import { useSyncCount } from "../SyncMonitor/hooks/useSyncCount";
import { useSyncMonitor } from "../SyncMonitor/hooks/useSyncMonitor";
import { AddGroupButton } from "./AddGroupButton";
import { useSpaceLayout, useSpaces } from "./hooks/useSpaces";
import { SpaceCustomizationPanel } from "./SpaceCustomizationPanel";
import { SpaceGroup } from "./SpaceGroup";
import { SpaceItem } from "./SpaceItem";
import { SpaceSwitcher } from "./SpaceSwitcher";

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
  const { active } = useDndContext();

  // Disable drop zone when dragging groups or space items (they have 'label' in their data)
  // This allows sortable collision detection to work for reordering
  const isDraggingSortableItem = active?.data?.current?.label != null;

  const { setNodeRef: setDropRef, isOver } = useDroppable({
    id: `space-root-before-${group.id}`,
    disabled: !spaceId || isDraggingSortableItem,
    data: {
      action: "add-to-space",
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
    setActivatorNodeRef,
  } = useSortable({
    id: group.id,
    data: {
      label: group.name,
    },
  });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
  };

  return (
    <div
      className={clsx("relative", isDragging && "z-50 opacity-50")}
      ref={setSortableRef}
      style={style}
    >
      {/* Drop zone before this group (for adding root-level items) */}
      <div
        className="absolute -top-2.5 right-0 left-0 z-10 h-5"
        ref={setDropRef}
      >
        {isOver && !isDragging && !isDraggingSortableItem && (
          <div className="absolute top-1/2 right-2 left-2 h-[2px] -translate-y-1/2 rounded-full bg-accent" />
        )}
      </div>
      <SpaceGroup
        group={group}
        items={items}
        sortableAttributes={attributes}
        sortableListeners={listeners}
        spaceId={spaceId}
      />
    </div>
  );
}

// Sync Monitor Button with Popover
function SyncButton() {
  const popover = usePopover();
  const navigate = useNavigate();
  const [showActivityFeed, setShowActivityFeed] = useState(false);
  const { onlinePeerCount, isSyncing } = useSyncCount();
  const sync = useSyncMonitor();

  useEffect(() => {
    if (popover.open) {
      setShowActivityFeed(false);
    }
  }, [popover.open]);

  const getStateColor = (state: string) => {
    switch (state) {
      case "Ready":
        return "bg-green-500";
      case "Backfilling":
        return "bg-yellow-500";
      case "CatchingUp":
        return "bg-accent";
      case "Uninitialized":
        return "bg-ink-faint";
      case "Paused":
        return "bg-ink-dull";
      default:
        return "bg-ink-faint";
    }
  };

  return (
    <Popover
      align="start"
      className="!p-0 !bg-app !rounded-xl z-50 max-h-[520px] w-[380px]"
      popover={popover}
      side="top"
      sideOffset={8}
      trigger={
        <TopBarButton
          icon={({ className, ...props }) =>
            isSyncing ? (
              <CircleNotch
                className={clsx(className, "animate-spin")}
                {...props}
              />
            ) : (
              <ArrowsClockwise className={className} {...props} />
            )
          }
          title="Sync Monitor"
        />
      }
    >
      <div className="flex items-center justify-between border-app-line border-b px-4 py-3">
        <h3 className="font-semibold text-ink text-sm">Sync Monitor</h3>

        <div className="flex items-center gap-2">
          {onlinePeerCount > 0 && (
            <span className="text-ink-dull text-xs">
              {onlinePeerCount} {onlinePeerCount === 1 ? "peer" : "peers"}{" "}
              online
            </span>
          )}

          <TopBarButton
            icon={ArrowsOut}
            onClick={() => navigate("/sync")}
            title="Open full sync monitor"
          />

          <TopBarButton
            active={showActivityFeed}
            icon={FunnelSimple}
            onClick={() => setShowActivityFeed(!showActivityFeed)}
            title={showActivityFeed ? "Show peers" : "Show activity feed"}
          />
        </div>
      </div>

      {popover.open && (
        <>
          <div className="border-app-line border-b bg-app-box/50 px-4 py-2">
            <div className="flex items-center gap-2">
              <div
                className={`size-2 rounded-full ${getStateColor(sync.currentState)}`}
              />
              <span className="font-medium text-ink-dull text-xs">
                {sync.currentState}
              </span>
            </div>
          </div>
          <motion.div
            animate={{
              height: showActivityFeed
                ? Math.min(sync.recentActivity.length * 40 + 16, 400)
                : Math.min(sync.peers.length * 80 + 16, 400),
            }}
            className="no-scrollbar overflow-y-auto"
            initial={false}
            transition={{ duration: 0.2, ease: [0.25, 1, 0.5, 1] }}
          >
            {showActivityFeed ? (
              <ActivityFeed activities={sync.recentActivity} />
            ) : (
              <PeerList currentState={sync.currentState} peers={sync.peers} />
            )}
          </motion.div>
        </>
      )}
    </Popover>
  );
}

// Jobs Button with Popover
function JobsButton({
  activeJobCount,
  hasRunningJobs,
  jobs,
  pause,
  resume,
  cancel,
  navigate,
}: {
  activeJobCount: number;
  hasRunningJobs: boolean;
  jobs: any[];
  pause: (jobId: string) => Promise<void>;
  resume: (jobId: string) => Promise<void>;
  cancel: (jobId: string) => Promise<void>;
  navigate: any;
}) {
  const popover = usePopover();
  const [showOnlyRunning, setShowOnlyRunning] = useState(true);

  useEffect(() => {
    if (popover.open) {
      setShowOnlyRunning(true);
    }
  }, [popover.open]);

  const filteredJobs = showOnlyRunning
    ? jobs.filter((job) => job.status === "running" || job.status === "paused")
    : jobs;

  return (
    <Popover
      align="start"
      className="!p-0 !bg-app !rounded-xl z-50 max-h-[480px] w-[360px]"
      popover={popover}
      side="top"
      sideOffset={8}
      trigger={
        <TopBarButton
          icon={({ className, ...props }) =>
            hasRunningJobs ? (
              <CircleNotch
                className={clsx(className, "animate-spin")}
                {...props}
              />
            ) : (
              <ListBullets className={className} {...props} />
            )
          }
          title="Job Manager"
        />
      }
    >
      <div className="flex items-center justify-between border-app-line border-b px-4 py-3">
        <h3 className="font-semibold text-ink text-sm">Job Manager</h3>

        <div className="flex items-center gap-2">
          {activeJobCount > 0 && (
            <span className="text-ink-dull text-xs">
              {activeJobCount} active
            </span>
          )}

          <TopBarButton
            icon={ArrowsOut}
            onClick={() => navigate("/jobs")}
            title="Open full jobs screen"
          />

          <TopBarButton
            active={showOnlyRunning}
            icon={FunnelSimple}
            onClick={() => setShowOnlyRunning(!showOnlyRunning)}
            title={showOnlyRunning ? "Show all jobs" : "Show only active jobs"}
          />
        </div>
      </div>

      {popover.open && (
        <motion.div
          animate={{
            height:
              filteredJobs.length === 0
                ? CARD_HEIGHT + 16
                : Math.min(filteredJobs.length * (CARD_HEIGHT + 8) + 16, 400),
          }}
          className="no-scrollbar overflow-y-auto"
          initial={false}
          transition={{ duration: 0.2, ease: [0.25, 1, 0.5, 1] }}
        >
          <JobList
            jobs={filteredJobs}
            onCancel={cancel}
            onPause={pause}
            onResume={resume}
          />
        </motion.div>
      )}
    </Popover>
  );
}

interface SpacesSidebarProps {
  isPreviewActive?: boolean;
}

export function SpacesSidebar({ isPreviewActive = false }: SpacesSidebarProps) {
  const client = useSpacedriveClient();
  const platform = usePlatform();
  const navigate = useNavigate();
  const { data: libraries } = useLibraries();
  const [currentLibraryId, setCurrentLibraryId] = useState<string | null>(() =>
    client.getCurrentLibraryId()
  );
  const [customizePanelOpen, setCustomizePanelOpen] = useState(false);

  // Get sync and job status for icons
  const { onlinePeerCount, isSyncing } = useSyncCount();
  const { activeJobCount, hasRunningJobs, jobs, pause, resume, cancel } =
    useJobs();

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
        platform
          .setCurrentLibraryId(firstLib.id)
          .catch((err) => console.error("Failed to set library ID:", err));
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
    <div className="flex h-full w-[220px] min-w-[176px] max-w-[300px] flex-col bg-transparent p-2">
      <div
        className={clsx(
          "flex h-full flex-col overflow-hidden rounded-2xl",
          isPreviewActive ? "bg-sidebar/80 backdrop-blur-2xl" : "bg-sidebar/65"
        )}
      >
        <nav className="relative z-[51] flex h-full flex-col gap-2.5 p-2.5 pt-[52px] pb-2">
          {/* Space Switcher */}
          <SpaceSwitcher
            currentSpace={currentSpace}
            onSwitch={setCurrentSpace}
            spaces={spaces}
          />

          {/* Scrollable Content */}
          <div className="no-scrollbar mask-fade-out mt-3 flex grow flex-col space-y-5 overflow-x-hidden overflow-y-scroll pb-10">
            {/* Space-level items (pinned shortcuts) */}
            {layout?.space_items && layout.space_items.length > 0 && (
              <SortableContext
                items={layout.space_items.map((item) => item.id)}
                strategy={verticalListSortingStrategy}
              >
                <div className="space-y-0.5">
                  {layout.space_items.map((item, index) => (
                    <SpaceItem
                      allowInsertion={true}
                      groupId={null}
                      isLastItem={index === layout.space_items.length - 1}
                      item={item}
                      key={item.id}
                      sortable={true}
                      spaceId={currentSpace?.id}
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
                    group={group}
                    isFirst={index === 0}
                    items={items}
                    key={group.id}
                    spaceId={currentSpace?.id}
                  />
                ))}
              </SortableContext>
            )}

            {/* Add Group Button */}
            {currentSpace && <AddGroupButton spaceId={currentSpace.id} />}
          </div>

          {/* Sync Monitor, Job Manager, Customize & Settings (pinned to bottom) */}
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <SyncButton />
              <JobsButton
                activeJobCount={activeJobCount}
                cancel={cancel}
                hasRunningJobs={hasRunningJobs}
                jobs={jobs}
                navigate={navigate}
                pause={pause}
                resume={resume}
              />
              <TopBarButton
                icon={Palette}
                onClick={() => setCustomizePanelOpen(true)}
                title="Customize"
              />
            </div>
            <TopBarButton
              icon={GearSix}
              onClick={() => {
                if (platform.showWindow) {
                  platform
                    .showWindow({ type: "Settings", page: "general" })
                    .catch((err) =>
                      console.error("Failed to open settings:", err)
                    );
                }
              }}
              title="Settings"
            />
          </div>
        </nav>
      </div>

      {/* Customization Panel */}
      <SpaceCustomizationPanel
        isOpen={customizePanelOpen}
        onClose={() => setCustomizePanelOpen(false)}
        spaceId={currentSpace?.id ?? null}
      />
    </div>
  );
}
