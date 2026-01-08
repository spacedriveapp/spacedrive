import { X } from "@phosphor-icons/react";
import type { File } from "@sd/ts-client";
import { useEffect, useState } from "react";
import { usePlatform } from "../../contexts/PlatformContext";
import { useNormalizedQuery } from "../../contexts/SpacedriveContext";
import { formatBytes, getContentKind } from "../../routes/explorer/utils";
import { ContentRenderer } from "./ContentRenderer";

function MetadataPanel({ file }: { file: File }) {
  return (
    <div className="w-[280px] min-w-[280px] overflow-y-auto border-sidebar-line border-l bg-sidebar-box p-4">
      <div className="space-y-4">
        <div>
          <div className="mb-1 text-ink-dull text-xs">Name</div>
          <div className="break-words text-ink text-sm">{file.name}</div>
        </div>

        <div>
          <div className="mb-1 text-ink-dull text-xs">Kind</div>
          <div className="text-ink text-sm capitalize">
            {getContentKind(file)}
          </div>
        </div>

        <div>
          <div className="mb-1 text-ink-dull text-xs">Size</div>
          <div className="text-ink text-sm">{formatBytes(file.size || 0)}</div>
        </div>

        {file.extension && (
          <div>
            <div className="mb-1 text-ink-dull text-xs">Extension</div>
            <div className="text-ink text-sm">{file.extension}</div>
          </div>
        )}

        {file.created_at && (
          <div>
            <div className="mb-1 text-ink-dull text-xs">Created</div>
            <div className="text-ink text-sm">
              {new Date(file.created_at).toLocaleString()}
            </div>
          </div>
        )}

        {file.modified_at && (
          <div>
            <div className="mb-1 text-ink-dull text-xs">Modified</div>
            <div className="text-ink text-sm">
              {new Date(file.modified_at).toLocaleString()}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

export function QuickPreview() {
  const platform = usePlatform();
  const [fileId, setFileId] = useState<string | null>(null);

  useEffect(() => {
    // Extract file_id from window label
    if (platform.getCurrentWindowLabel) {
      const label = platform.getCurrentWindowLabel();

      // Label format: "quick-preview-{file_id}"
      const match = label.match(/^quick-preview-(.+)$/);
      if (match) {
        setFileId(match[1]);
      }
    }
  }, [platform]);

  const {
    data: file,
    isLoading,
    error,
  } = useNormalizedQuery<{ file_id: string }, File>({
    wireMethod: "query:files.by_id",
    input: { file_id: fileId! },
    resourceType: "file",
    resourceId: fileId!,
    enabled: !!fileId,
  });

  const handleClose = () => {
    if (platform.closeCurrentWindow) {
      platform.closeCurrentWindow();
    }
  };

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.code === "Escape") {
        handleClose();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  if (isLoading || !file) {
    return (
      <div className="flex h-screen items-center justify-center bg-app text-ink">
        <div className="animate-pulse">Loading...</div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex h-screen items-center justify-center bg-app text-red-400">
        <div>
          <div className="mb-2 font-medium text-lg">Error loading file</div>
          <div className="text-sm">{error.message}</div>
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-screen flex-col bg-app text-ink">
      {/* Header */}
      <div className="flex items-center justify-between border-app-line border-b px-4 py-3">
        <div className="flex-1 truncate font-medium text-sm">{file.name}</div>
        <button
          className="rounded-md p-1 text-ink-dull hover:bg-app-hover hover:text-ink"
          onClick={handleClose}
        >
          <X size={16} weight="bold" />
        </button>
      </div>

      {/* Content Area */}
      <div className="flex flex-1 overflow-hidden">
        {/* File Content */}
        <div className="flex-1 bg-app-box/30 p-6">
          <ContentRenderer file={file} />
        </div>

        {/* Metadata Sidebar */}
        <MetadataPanel file={file} />
      </div>

      {/* Footer with keyboard hints */}
      <div className="border-app-line border-t bg-app-box/30 px-4 py-2">
        <div className="text-center text-ink-dull text-xs">
          Press <span className="text-ink">ESC</span> to close
        </div>
      </div>
    </div>
  );
}
