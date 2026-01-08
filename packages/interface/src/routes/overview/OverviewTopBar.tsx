import {
  ArrowsClockwise,
  CaretDown,
  CloudArrowUp,
  DeviceMobile,
  GearSix,
  MagnifyingGlass,
  Plus,
} from "@phosphor-icons/react";
import { useLibraryMutation } from "@sd/ts-client";
import { Popover, TopBarButton, usePopover } from "@sd/ui";
import clsx from "clsx";
import { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useCreateLibraryDialog } from "../../components/modals/CreateLibraryModal";
import { PairingModal } from "../../components/modals/PairingModal";
import { useSyncSetupDialog } from "../../components/modals/SyncSetupModal";
import { usePlatform } from "../../contexts/PlatformContext";
import { useSpacedriveClient } from "../../contexts/SpacedriveContext";
import { useLibraries } from "../../hooks/useLibraries";
import { TopBarItem, TopBarPortal } from "../../TopBar";
import { useAddStorageDialog } from "../explorer/components/AddStorageModal";

interface OverviewTopBarProps {
  libraryName?: string;
}

export function OverviewTopBar({ libraryName }: OverviewTopBarProps) {
  const [isPairingOpen, setIsPairingOpen] = useState(false);
  const navigate = useNavigate();
  const client = useSpacedriveClient();
  const platform = usePlatform();
  const { data: libraries } = useLibraries();
  const [currentLibraryId, setCurrentLibraryId] = useState<string | null>(
    () => client.getCurrentLibraryId() // Initialize from client
  );
  const librarySwitcher = usePopover();

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

  const handleLibrarySwitch = (libraryId: string) => {
    librarySwitcher.setOpen(false);

    // Set library ID via platform (syncs to all windows on Tauri)
    if (platform.setCurrentLibraryId) {
      platform
        .setCurrentLibraryId(libraryId)
        .catch((err) => console.error("Failed to set library ID:", err));
    } else {
      // Web fallback - just update client
      client.setCurrentLibrary(libraryId);
    }
  };

  const currentLibrary = libraries?.find((lib) => lib.id === currentLibraryId);

  const handleAddStorage = () => {
    useAddStorageDialog((id) => {
      navigate(`/location/${id}`);
    });
  };

  const handleSyncSetup = () => {
    useSyncSetupDialog();
  };

  // Mutation for refreshing volume statistics
  const volumeRefreshMutation = useLibraryMutation("volumes.refresh");
  const [isRefreshing, setIsRefreshing] = useState(false);

  const handleRefresh = async () => {
    setIsRefreshing(true);
    try {
      const result = (await volumeRefreshMutation.mutateAsync({
        force: false,
      } as any)) as any;
      console.log(
        `Volume refresh complete: ${result.volumes_refreshed} refreshed, ${result.volumes_failed} failed`
      );
    } catch (error) {
      console.error("Failed to refresh volumes:", error);
    } finally {
      setIsRefreshing(false);
    }
  };

  // Memoize TopBarItem children to prevent infinite re-renders
  const overviewTitleContent = useMemo(
    () => (
      <div className="flex items-center gap-3">
        <h1 className="font-bold text-ink text-xl">Overview</h1>
        <span className="text-ink-dull">•</span>
        <Popover
          className="min-w-[200px] p-2"
          popover={librarySwitcher}
          trigger={
            <button
              className={clsx(
                "flex h-8 items-center gap-2 rounded-full px-3 font-medium text-xs",
                "backdrop-blur-xl transition-all",
                "border border-sidebar-line/30",
                "bg-sidebar-box/20 text-sidebar-inkDull hover:bg-sidebar-box/30 hover:text-sidebar-ink",
                "active:scale-95",
                !currentLibrary && "text-ink-faint"
              )}
            >
              <span className="max-w-[200px] truncate">
                {currentLibrary?.name || libraryName || "Select Library"}
              </span>
              <CaretDown size={12} weight="bold" />
            </button>
          }
        >
          <div className="space-y-1">
            {libraries && libraries.length > 1 && (
              <>
                {libraries.map((lib) => (
                  <button
                    className={clsx(
                      "w-full cursor-pointer rounded-md px-3 py-2 text-left text-sm",
                      lib.id === currentLibraryId
                        ? "bg-accent text-white"
                        : "text-ink hover:bg-app-selected"
                    )}
                    key={lib.id}
                    onClick={() => handleLibrarySwitch(lib.id)}
                  >
                    {lib.name}
                  </button>
                ))}
                <div className="my-1 border-app-line border-t" />
              </>
            )}
            <button
              className="flex w-full cursor-pointer items-center gap-2 rounded-md px-3 py-2 font-medium text-ink text-sm hover:bg-app-selected"
              onClick={() => {
                librarySwitcher.setOpen(false);
                useCreateLibraryDialog();
              }}
            >
              <Plus size={16} weight="bold" />
              <span>New Library</span>
            </button>
            <button
              className="flex w-full cursor-pointer items-center gap-2 rounded-md px-3 py-2 font-medium text-ink text-sm hover:bg-app-selected"
              onClick={() => librarySwitcher.setOpen(false)}
            >
              <GearSix size={16} weight="bold" />
              <span>Library Settings</span>
            </button>
          </div>
        </Popover>
      </div>
    ),
    [
      libraries,
      currentLibrary,
      libraryName,
      currentLibraryId,
      librarySwitcher,
      handleLibrarySwitch,
    ]
  );

  const searchButton = useMemo(
    () => <TopBarButton icon={MagnifyingGlass} title="Search" />,
    []
  );

  const pairButton = useMemo(
    () => (
      <TopBarButton
        icon={DeviceMobile}
        onClick={() => setIsPairingOpen(true)}
        title="Pair Device"
      >
        Pair
      </TopBarButton>
    ),
    []
  );

  const syncButton = useMemo(
    () => (
      <TopBarButton
        icon={CloudArrowUp}
        onClick={handleSyncSetup}
        title="Setup Sync"
      >
        Setup Sync
      </TopBarButton>
    ),
    [handleSyncSetup]
  );

  const refreshButton = useMemo(
    () => (
      <TopBarButton
        className={clsx(isRefreshing && "animate-spin")}
        disabled={isRefreshing}
        icon={ArrowsClockwise}
        onClick={handleRefresh}
        title="Refresh Statistics"
      >
        Refresh
      </TopBarButton>
    ),
    [handleRefresh, isRefreshing]
  );

  const addStorageButton = useMemo(
    () => (
      <TopBarButton
        className="!bg-accent hover:!bg-accent-deep !text-white"
        icon={Plus}
        onClick={handleAddStorage}
      >
        Add Storage
      </TopBarButton>
    ),
    [handleAddStorage]
  );

  return (
    <>
      <TopBarPortal
        left={
          <>
            <TopBarItem id="overview-title" label="Overview" priority="high">
              {overviewTitleContent}
            </TopBarItem>
          </>
        }
        right={
          <>
            <TopBarItem id="search" label="Search" priority="high">
              {searchButton}
            </TopBarItem>
            <TopBarItem
              id="pair-device"
              label="Pair Device"
              onClick={() => setIsPairingOpen(true)}
              priority="normal"
            >
              {pairButton}
            </TopBarItem>
            <TopBarItem
              id="setup-sync"
              label="Setup Sync"
              onClick={handleSyncSetup}
              priority="low"
            >
              {syncButton}
            </TopBarItem>
            <TopBarItem
              id="refresh"
              label="Refresh Statistics"
              onClick={handleRefresh}
              priority="low"
            >
              {refreshButton}
            </TopBarItem>
            <TopBarItem
              id="add-storage"
              label="Add Storage"
              onClick={handleAddStorage}
              priority="high"
            >
              {addStorageButton}
            </TopBarItem>
          </>
        }
      />

      <PairingModal
        isOpen={isPairingOpen}
        onClose={() => setIsPairingOpen(false)}
      />
    </>
  );
}
