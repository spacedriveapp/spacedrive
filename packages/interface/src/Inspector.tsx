import { ArrowSquareOut } from "@phosphor-icons/react";
import { useEffect, useState } from "react";
import type { File, LocationInfo } from "@sd/ts-client/generated/types";
import { useLibraryQuery } from "./context";
import { usePlatform } from "./platform";
import { FileInspector } from "./inspectors/FileInspector";
import { LocationInspector } from "./inspectors/LocationInspector";

export type InspectorVariant =
  | { type: "file"; file: File }
  | { type: "location"; location: LocationInfo }
  | { type: "empty" }
  | null;

interface InspectorProps {
  variant: InspectorVariant;
  onPopOut?: () => void;
  showPopOutButton?: boolean;
}

export function Inspector({
  variant,
  onPopOut,
  showPopOutButton = true,
}: InspectorProps) {
  // Note: Window styling is now handled by the Tauri app layer
  // No need for interface package to call platform-specific commands

  return (
    <div className="flex flex-col h-full rounded-2xl overflow-hidden bg-sidebar/65">
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
    if (platform.onSelectedFilesChanged) {
      let unlisten: (() => void) | undefined;

      platform.onSelectedFilesChanged((fileIds) => {
        setSelectedFileIds(fileIds);
      }).then((unlistenFn) => {
        unlisten = unlistenFn;
      }).catch((err) => {
        console.error("Failed to listen for selected files changes:", err);
      });

      return () => {
        unlisten?.();
      };
    }
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

  return <Inspector variant={variant} showPopOutButton={false} />;
}
