import { ArrowSquareOut } from "@phosphor-icons/react";
import { useEffect, useState, useMemo } from "react";
import { useParams } from "react-router-dom";
import type { File, LocationInfo } from "@sd/ts-client";
import { useLibraryQuery, useNormalizedQuery } from "./context";
import { usePlatform } from "./platform";
import { useSelection } from "./components/Explorer/SelectionContext";
import { FileInspector } from "./inspectors/FileInspector";
import { LocationInspector } from "./inspectors/LocationInspector";
import { isVirtualFile } from "./components/Explorer/utils/virtualFiles";
import clsx from "clsx";

export type InspectorVariant =
  | { type: "file"; file: File }
  | { type: "location"; location: LocationInfo }
  | { type: "empty" }
  | null;

interface InspectorProps {
  onPopOut?: () => void;
  showPopOutButton?: boolean;
  currentLocation?: LocationInfo | null;
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
    if (selectedFiles.length > 0 && selectedFiles[0]) {
      const file = selectedFiles[0];

      // Check if this is a virtual location file
      if (isVirtualFile(file) && (file as any)._virtual?.type === "location") {
        // Show LocationInspector for virtual locations
        const locationData = (file as any)._virtual.data as LocationInfo;
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
      variant={variant}
      onPopOut={onPopOut}
      showPopOutButton={showPopOutButton}
      isPreviewActive={isPreviewActive}
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
        "flex flex-col h-full rounded-2xl overflow-hidden",
        isPreviewActive ? "backdrop-blur-2xl bg-sidebar/80" : "bg-sidebar/65",
      )}
    >
      <div className="relative z-[51] flex h-full flex-col p-2.5 pb-2">
        {/* Variant-specific content */}
        {!variant || variant.type === "empty" ? (
          <EmptyState />
        ) : variant.type === "file" ? (
          <FileInspector file={variant.file} />
        ) : variant.type === "location" ? (
          <LocationInspector location={variant.location} />
        ) : null}

        {/* Footer with pop-out button */}
        {showPopOutButton && onPopOut && (
          <div className="border-t border-sidebar-line pt-2 flex justify-center mt-2.5">
            <button
              onClick={onPopOut}
              className="p-1.5 rounded-lg hover:bg-sidebar-selected transition-colors"
              title="Pop out Inspector"
            >
              <ArrowSquareOut
                className="size-4 text-sidebar-inkDull hover:text-sidebar-ink transition-colors"
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
    <div className="flex-1 flex items-center justify-center px-4 text-center">
      <p className="text-xs text-sidebar-inkDull">
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
      platform.getSelectedFileIds()
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

    platform.onSelectedFilesChanged((fileIds) => {
      if (mounted) {
        setSelectedFileIds(fileIds);
      }
    }).then((unlistenFn) => {
      if (mounted) {
        unlisten = unlistenFn;
      } else {
        unlistenFn();
      }
    }).catch((err) => {
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
      <div className="flex flex-col h-full rounded-2xl overflow-hidden bg-sidebar/65">
        <div className="flex-1 flex items-center justify-center">
          <p className="text-xs text-sidebar-inkDull">Loading...</p>
        </div>
      </div>
    );
  }

  return <InspectorView variant={variant} showPopOutButton={false} />;
}
