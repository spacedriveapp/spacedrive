import {
  CaretDown,
  Clock,
  GearSix,
  Heart,
  Network,
  Planet,
  Plus,
  Tag,
} from "@phosphor-icons/react";
import { DropdownMenu } from "@sd/ui";
import clsx from "clsx";
import { useEffect, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { JobManagerPopover } from "../../components/JobManager";
import { usePlatform } from "../../contexts/PlatformContext";
import { useSpacedriveClient } from "../../contexts/SpacedriveContext";
import { useLibraries } from "../../hooks/useLibraries";
import { LocationsSection } from "./components/LocationsSection";
import { Section } from "./components/Section";
import { SidebarItem } from "./components/SidebarItem";

export function Sidebar() {
  const client = useSpacedriveClient();
  const platform = usePlatform();
  const { data: libraries } = useLibraries();
  const navigate = useNavigate();
  const location = useLocation();
  const [currentLibraryId, setCurrentLibraryId] = useState<string | null>(() =>
    client.getCurrentLibraryId()
  );

  const isActive = (path: string) => location.pathname === path;

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

  return (
    <div className="flex h-full w-[220px] min-w-[176px] max-w-[300px] flex-col bg-app p-2">
      <div
        className={clsx(
          "flex h-full flex-col overflow-hidden rounded-2xl",
          "bg-sidebar/65"
        )}
      >
        <nav className="relative z-[51] flex h-full flex-col gap-2.5 p-2.5 pt-[52px] pb-2">
          <DropdownMenu.Root
            className="overflow-hidden rounded-lg border border-sidebar-line bg-sidebar-box p-1 shadow-sm"
            trigger={
              <button
                className={clsx(
                  "flex w-full items-center gap-1.5 rounded-lg px-2 py-1.5 font-medium text-sm",
                  "border border-sidebar-line bg-sidebar-box",
                  "text-sidebar-ink hover:bg-sidebar-button",
                  "focus:outline-none focus:ring-1 focus:ring-accent",
                  "transition-colors",
                  !currentLibrary && "text-sidebar-inkFaint"
                )}
              >
                <span className="flex-1 truncate text-left">
                  {currentLibrary?.name || "Select Library"}
                </span>
                <CaretDown className="size-3 opacity-50" />
              </button>
            }
          >
            {libraries && libraries.length > 1
              ? libraries.map((lib) => (
                  <DropdownMenu.Item
                    className={clsx(
                      "rounded-md px-2 py-1 text-sm",
                      lib.id === currentLibraryId
                        ? "bg-accent text-white"
                        : "text-sidebar-ink hover:bg-sidebar-selected"
                    )}
                    key={lib.id}
                    onClick={() => handleLibrarySwitch(lib.id)}
                  >
                    {lib.name}
                  </DropdownMenu.Item>
                ))
              : null}
            {libraries && libraries.length > 1 && (
              <DropdownMenu.Separator className="my-1 border-sidebar-line" />
            )}
            <DropdownMenu.Item
              className="rounded-md px-2 py-1 font-medium text-sidebar-ink text-sm hover:bg-sidebar-selected"
              icon={Plus}
            >
              New Library
            </DropdownMenu.Item>
            <DropdownMenu.Item
              className="rounded-md px-2 py-1 font-medium text-sidebar-ink text-sm hover:bg-sidebar-selected"
              icon={GearSix}
            >
              Library Settings
            </DropdownMenu.Item>
          </DropdownMenu.Root>

          <div className="no-scrollbar mask-fade-out flex grow flex-col space-y-5 overflow-x-hidden overflow-y-scroll pb-10">
            <div className="space-y-0.5">
              <SidebarItem
                active={isActive("/")}
                icon={Planet}
                label="Overview"
                onClick={() => navigate("/")}
                weight={isActive("/") ? "fill" : "bold"}
              />
              <SidebarItem
                active={isActive("/recents")}
                icon={Clock}
                label="Recents"
                onClick={() => navigate("/recents")}
              />
              <SidebarItem
                active={isActive("/favorites")}
                icon={Heart}
                label="Favorites"
                onClick={() => navigate("/favorites")}
              />
            </div>

            <LocationsSection />

            <Section title="Tags">
              <SidebarItem color="#3B82F6" icon={Tag} label="Work" />
              <SidebarItem color="#10B981" icon={Tag} label="Personal" />
              <SidebarItem color="#F59E0B" icon={Tag} label="Archive" />
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
