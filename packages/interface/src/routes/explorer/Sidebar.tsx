import { useState, useEffect } from "react";
import clsx from "clsx";
import {
  House,
  Clock,
  Heart,
  Tag,
  Network,
  GearSix,
  Planet,
  CaretDown,
  Plus,
} from "@phosphor-icons/react";
import { DropdownMenu } from "@sd/ui";
import { useSpacedriveClient } from "../../contexts/SpacedriveContext";
import { useLibraries } from "../../hooks/useLibraries";
import { usePlatform } from "../../contexts/PlatformContext";
import { LocationsSection } from "./components/LocationsSection";
import { Section } from "./components/Section";
import { SidebarItem } from "./components/SidebarItem";
import { JobManagerPopover } from "../../components/JobManager";
import { SyncMonitorPopover } from "../../components/SyncMonitor";

export function Sidebar() {
  const client = useSpacedriveClient();
  const platform = usePlatform();
  const { data: libraries } = useLibraries();
  const [currentLibraryId, setCurrentLibraryId] = useState<string | null>(
    () => client.getCurrentLibraryId(),
  );

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

  const handleLibrarySwitch = (libraryId: string) => {
    // Set library ID via platform (syncs to all windows on Tauri)
    if (platform.setCurrentLibraryId) {
      platform.setCurrentLibraryId(libraryId).catch((err) =>
        console.error("Failed to set library ID:", err),
      );
    } else {
      // Web fallback - just update client
      client.setCurrentLibrary(libraryId);
    }
  };

  const currentLibrary = libraries?.find((lib) => lib.id === currentLibraryId);

  return (
    <div className="w-[220px] min-w-[176px] max-w-[300px] flex flex-col h-full p-2 bg-app">
      <div
        className={clsx(
          "flex flex-col h-full rounded-2xl overflow-hidden",
          "bg-sidebar/65",
        )}
      >
        <nav className="relative z-[51] flex h-full flex-col gap-2.5 p-2.5 pb-2 pt-[52px]">
          <DropdownMenu.Root
            trigger={
              <button
                className={clsx(
                  "w-full flex items-center gap-1.5 rounded-lg px-2 py-1.5 text-sm font-medium",
                  "bg-sidebar-box border border-sidebar-line",
                  "text-sidebar-ink hover:bg-sidebar-button",
                  "focus:outline-none focus:ring-1 focus:ring-accent",
                  "transition-colors",
                  !currentLibrary && "text-sidebar-inkFaint",
                )}
              >
                <span className="truncate flex-1 text-left">
                  {currentLibrary?.name || "Select Library"}
                </span>
                <CaretDown className="size-3 opacity-50" />
              </button>
            }
            className="p-1 bg-sidebar-box border border-sidebar-line rounded-lg shadow-sm overflow-hidden"
          >
            {libraries && libraries.length > 1
              ? libraries.map((lib) => (
                  <DropdownMenu.Item
                    key={lib.id}
                    onClick={() => handleLibrarySwitch(lib.id)}
                    className={clsx(
                      "px-2 py-1 text-sm rounded-md",
                      lib.id === currentLibraryId
                        ? "bg-accent text-white"
                        : "text-sidebar-ink hover:bg-sidebar-selected",
                    )}
                  >
                    {lib.name}
                  </DropdownMenu.Item>
                ))
              : null}
            {libraries && libraries.length > 1 && (
              <DropdownMenu.Separator className="border-sidebar-line my-1" />
            )}
            <DropdownMenu.Item
              icon={Plus}
              className="px-2 py-1 text-sm rounded-md hover:bg-sidebar-selected text-sidebar-ink font-medium"
            >
              New Library
            </DropdownMenu.Item>
            <DropdownMenu.Item
              icon={GearSix}
              className="px-2 py-1 text-sm rounded-md hover:bg-sidebar-selected text-sidebar-ink font-medium"
            >
              Library Settings
            </DropdownMenu.Item>
          </DropdownMenu.Root>

          <div className="no-scrollbar mask-fade-out flex grow flex-col space-y-5 overflow-x-hidden overflow-y-scroll pb-10">
            <div className="space-y-0.5">
              <SidebarItem icon={Planet} label="Overview" to="/" end />
              <SidebarItem icon={Clock} label="Recents" to="/recents" />
              <SidebarItem icon={Heart} label="Favorites" to="/favorites" />
            </div>

            <LocationsSection />

            <Section title="Tags">
              <SidebarItem icon={Tag} label="Work" color="#3B82F6" />
              <SidebarItem icon={Tag} label="Personal" color="#10B981" />
              <SidebarItem icon={Tag} label="Archive" color="#F59E0B" />
            </Section>

            <Section title="Cloud">
              <SidebarItem icon={Network} label="Sync" />
            </Section>
          </div>

          <div className="space-y-0.5">
            <SidebarItem icon={GearSix} label="Settings" />
          </div>

          <div className="mt-2">
            <JobManagerPopover />
          </div>
        </nav>
      </div>
    </div>
  );
}