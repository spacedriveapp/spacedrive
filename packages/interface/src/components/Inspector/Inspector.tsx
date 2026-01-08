import { ArrowSquareOut } from "@phosphor-icons/react";
import type { File, Location } from "@sd/ts-client";
import clsx from "clsx";
import { useEffect, useMemo, useState } from "react";
import { usePlatform } from "../../contexts/PlatformContext";
import { useLibraryQuery } from "../../contexts/SpacedriveContext";
import { useSelection } from "../../routes/explorer/SelectionContext";
import { isVirtualFile } from "../../routes/explorer/utils/virtualFiles";
import { FileInspector } from "./variants/FileInspector";
import { LocationInspector } from "./variants/LocationInspector";
import { MultiFileInspector } from "./variants/MultiFileInspector";

// Re-export primitives for convenience
export {
  Divider,
  InfoRow,
  Section,
  TabContent,
  Tabs,
  Tag,
} from "./primitives";

export type InspectorVariant =
  | { type: "file"; file: File }
  | { type: "multi-file"; files: File[] }
  | { type: "location"; location: Location }
  | { type: "empty" }
  | null;

interface InspectorProps {
  onPopOut?: () => void;
  showPopOutButton?: boolean;
  currentLocation?: Location | null;
  isPreviewActive?: boolean;
}

export function Inspector({
  onPopOut,
  showPopOutButton = true,
  currentLocation,
  isPreviewActive = false,
}: InspectorProps) {
  const { selectedFiles } = useSelection();

  // Compute inspector variant based on selection
  const variant: InspectorVariant = useMemo(() => {
    if (selectedFiles.length > 1) {
      // Multiple files selected
      return { type: "multi-file", files: selectedFiles };
    }
    if (selectedFiles.length > 0 && selectedFiles[0]) {
      const file = selectedFiles[0];

      // Check if this is a virtual location file
      if (isVirtualFile(file) && (file as any)._virtual?.type === "location") {
        // Show LocationInspector for virtual locations
        const locationData = (file as any)._virtual.data as Location;
        return { type: "location", location: locationData };
      }

      // Regular file
      return { type: "file", file };
    }
    if (currentLocation) {
      return { type: "location", location: currentLocation };
    }
    return { type: "empty" };
  }, [selectedFiles, currentLocation]);

  return (
    <InspectorView
      isPreviewActive={isPreviewActive}
      onPopOut={onPopOut}
      showPopOutButton={showPopOutButton}
      variant={variant}
    />
  );
}

interface InspectorViewProps {
  variant: InspectorVariant;
  onPopOut?: () => void;
  showPopOutButton?: boolean;
  isPreviewActive?: boolean;
}

function InspectorView({
  variant,
  onPopOut,
  showPopOutButton = true,
  isPreviewActive = false,
}: InspectorViewProps) {
  return (
    <div
      className={clsx(
        "flex h-full flex-col overflow-hidden rounded-2xl",
        isPreviewActive ? "bg-sidebar/80 backdrop-blur-2xl" : "bg-sidebar/65"
      )}
    >
      <div className="relative z-[51] flex h-full flex-col p-2.5 pb-2">
        {/* Variant-specific content */}
        {!variant || variant.type === "empty" ? (
          <EmptyState />
        ) : variant.type === "file" ? (
          <FileInspector file={variant.file} />
        ) : variant.type === "multi-file" ? (
          <MultiFileInspector files={variant.files} />
        ) : variant.type === "location" ? (
          <LocationInspector location={variant.location} />
        ) : null}

        {/* Footer with pop-out button */}
        {showPopOutButton && onPopOut && (
          <div className="mt-2.5 flex justify-center border-sidebar-line border-t pt-2">
            <button
              className="rounded-lg p-1.5 transition-colors hover:bg-sidebar-selected"
              onClick={onPopOut}
              title="Pop out Inspector"
            >
              <ArrowSquareOut
                className="size-4 text-sidebar-inkDull transition-colors hover:text-sidebar-ink"
                weight="bold"
              />
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

function EmptyState() {
  return (
    <div className="flex flex-1 items-center justify-center px-4 text-center">
      <p className="text-sidebar-inkDull text-xs">
        Select an item to view details
      </p>
    </div>
  );
}

/**
 * Popout Inspector - Queries selected files from platform state
 * This is used when the inspector is opened in a separate window
 */
export function PopoutInspector() {
  const platform = usePlatform();
  const [selectedFileIds, setSelectedFileIds] = useState<string[]>([]);

  // Query selected file IDs from platform on mount
  useEffect(() => {
    if (platform.getSelectedFileIds) {
      platform
        .getSelectedFileIds()
        .then((fileIds) => {
          setSelectedFileIds(fileIds);
        })
        .catch((err) => {
          console.error("Failed to get selected file IDs:", err);
        });
    }
  }, [platform]);

  // Listen for selection changes from main window
  useEffect(() => {
    if (!platform.onSelectedFilesChanged) return;

    let unlisten: (() => void) | undefined;
    let mounted = true;

    platform
      .onSelectedFilesChanged((fileIds) => {
        if (mounted) {
          setSelectedFileIds(fileIds);
        }
      })
      .then((unlistenFn) => {
        if (mounted) {
          unlisten = unlistenFn;
        } else {
          unlistenFn();
        }
      })
      .catch((err) => {
        console.error("Failed to listen for selected files changes:", err);
      });

    return () => {
      mounted = false;
      unlisten?.();
    };
  }, [platform]);

  // Fetch the first selected file
  const firstFileId = selectedFileIds[0] || null;

  const { data: file, isLoading } = useLibraryQuery(
    {
      type: "files.by_id",
      input: { file_id: firstFileId! },
    },
    {
      enabled: !!firstFileId,
    }
  );

  // Compute inspector variant
  const variant: InspectorVariant = file
    ? { type: "file", file }
    : selectedFileIds.length > 0
      ? { type: "empty" } // Loading state
      : { type: "empty" }; // No selection

  if (isLoading) {
    return (
      <div className="flex h-full flex-col overflow-hidden rounded-2xl bg-sidebar/65">
        <div className="flex flex-1 items-center justify-center">
          <p className="text-sidebar-inkDull text-xs">Loading...</p>
        </div>
      </div>
    );
  }

  return <InspectorView showPopOutButton={false} variant={variant} />;
}
