import { useRef, useMemo } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import clsx from "clsx";
import type { File, SdPath } from "@sd/ts-client/generated/types";
import { useNormalizedCache } from "../../../../context";
import { ColumnItem } from "./ColumnItem";
import { useExplorer } from "../../context";
import { useContextMenu } from "../../../../hooks/useContextMenu";
import { Copy, Trash, Eye, FolderOpen } from "@phosphor-icons/react";
import { useLibraryMutation } from "../../../../context";

interface ColumnProps {
  path: SdPath;
  isActive: boolean;
  onNavigate: (path: SdPath) => void;
}

export function Column({ path, isActive, onNavigate }: ColumnProps) {
  const parentRef = useRef<HTMLDivElement>(null);
  const { selectFile, selectedFiles, focusedIndex, currentPath, viewSettings } = useExplorer();
  const copyFiles = useLibraryMutation("files.copy");
  const deleteFiles = useLibraryMutation("files.delete");

  const directoryQuery = useNormalizedCache({
    wireMethod: "query:files.directory_listing",
    input: {
      path: path,
      limit: null,
      include_hidden: false,
      sort_by: "name",
    },
    resourceType: "file",
    resourceFilter: (file: any) => {
      if (!file.sd_path) return false;

      const filePath = file.sd_path;

      if (filePath.Physical && path?.Physical) {
        if (filePath.Physical.device_slug !== path.Physical.device_slug) {
          return false;
        }

        const filePathStr = filePath.Physical.path;
        const scopePathStr = path.Physical.path;
        const fileParent = filePathStr.substring(0, filePathStr.lastIndexOf('/'));

        return fileParent === scopePathStr;
      }

      if (filePath.Content && path?.Physical) {
        const alternates = file.alternate_paths || [];

        for (const altPath of alternates) {
          if (altPath.Physical) {
            if (altPath.Physical.device_slug !== path.Physical.device_slug) {
              continue;
            }

            const altPathStr = altPath.Physical.path;
            const scopePathStr = path.Physical.path;
            const altParent = altPathStr.substring(0, altPathStr.lastIndexOf('/'));

            if (altParent === scopePathStr) {
              return true;
            }
          }
        }

        return false;
      }

      return false;
    },
  });

  const files = directoryQuery.data?.files || [];

  const rowVirtualizer = useVirtualizer({
    count: files.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 32,
    overscan: 10,
  });

  const getTargetFiles = (file: File) => {
    const isSelected = selectedFiles.some((f) => f.id === file.id);
    if (isSelected && selectedFiles.length > 0) {
      return selectedFiles;
    }
    return [file];
  };

  const contextMenu = useContextMenu({
    items: [
      {
        icon: Eye,
        label: "Quick Look",
        onClick: () => {
          console.log("Quick Look");
        },
        keybind: "Space",
      },
      {
        icon: FolderOpen,
        label: "Open",
        onClick: (file: File) => {
          if (file.kind === "Directory") {
            onNavigate(file.sd_path);
          }
        },
        keybind: "⌘O",
      },
      { type: "separator" },
      {
        icon: Copy,
        label: selectedFiles.length > 1 ? `Copy ${selectedFiles.length} items` : "Copy",
        onClick: async () => {
          const sdPaths = selectedFiles.map((f) => f.sd_path);
          window.__SPACEDRIVE__ = window.__SPACEDRIVE__ || {};
          window.__SPACEDRIVE__.clipboard = {
            operation: 'copy',
            files: sdPaths,
            sourcePath: currentPath,
          };
        },
        keybind: "⌘C",
        condition: () => selectedFiles.length > 0,
      },
      {
        icon: Copy,
        label: "Paste",
        onClick: async () => {
          const clipboard = window.__SPACEDRIVE__?.clipboard;
          if (!clipboard || !clipboard.files || !currentPath) {
            return;
          }

          try {
            await copyFiles.mutateAsync({
              sources: { paths: clipboard.files },
              destination: currentPath,
              overwrite: false,
              verify_checksum: false,
              preserve_timestamps: true,
              move_files: false,
              copy_method: "Auto" as const,
            });
          } catch (err) {
            console.error("Failed to paste:", err);
          }
        },
        keybind: "⌘V",
        condition: () => {
          const clipboard = window.__SPACEDRIVE__?.clipboard;
          return !!clipboard && !!clipboard.files && clipboard.files.length > 0;
        },
      },
      { type: "separator" },
      {
        icon: Trash,
        label: selectedFiles.length > 1 ? `Delete ${selectedFiles.length} items` : "Delete",
        onClick: async (file: File) => {
          const targets = getTargetFiles(file);
          const message = targets.length > 1
            ? `Delete ${targets.length} items?`
            : `Delete "${file.name}"?`;

          if (confirm(message)) {
            try {
              await deleteFiles.mutateAsync({
                targets: { paths: targets.map((f) => f.sd_path) },
                permanent: false,
                recursive: true,
              });
            } catch (err) {
              console.error("Failed to delete:", err);
            }
          }
        },
        keybind: "⌘⌫",
        variant: "danger" as const,
      },
    ],
  });

  const handleItemClick = (file: File, multi?: boolean, range?: boolean) => {
    selectFile(file, multi, range);
    // Don't navigate here - let the selection trigger column addition in ColumnView
  };

  const handleContextMenu = async (e: React.MouseEvent, file: File) => {
    e.preventDefault();
    e.stopPropagation();

    const isSelected = selectedFiles.some((f) => f.id === file.id);
    if (!isSelected) {
      selectFile(file, false, false);
    }

    await contextMenu.show(e);
  };

  if (directoryQuery.isLoading) {
    return (
      <div
        className="shrink-0 border-r border-app-line flex items-center justify-center"
        style={{ width: `${viewSettings.columnWidth}px` }}
      >
        <div className="text-sm text-ink-dull">Loading...</div>
      </div>
    );
  }

  return (
    <div
      ref={parentRef}
      className={clsx(
        "shrink-0 border-r border-app-line overflow-auto",
        isActive && "bg-app-box/30"
      )}
      style={{ width: `${viewSettings.columnWidth}px` }}
    >
      {files.length === 0 ? (
        <div className="p-4 text-sm text-ink-dull">Empty folder</div>
      ) : (
        <div
          style={{
            height: `${rowVirtualizer.getTotalSize()}px`,
            width: '100%',
            position: 'relative',
          }}
        >
          {rowVirtualizer.getVirtualItems().map((virtualRow) => {
            const file = files[virtualRow.index];
            const isSelected = selectedFiles.some((f) => f.id === file.id);
            const isFocused = focusedIndex === virtualRow.index;

            return (
              <div
                key={virtualRow.key}
                style={{
                  position: 'absolute',
                  top: 0,
                  left: 0,
                  width: '100%',
                  height: `${virtualRow.size}px`,
                  transform: `translateY(${virtualRow.start}px)`,
                }}
              >
                <ColumnItem
                  file={file}
                  selected={isSelected}
                  focused={isFocused}
                  onClick={handleItemClick}
                  onContextMenu={(e) => handleContextMenu(e, file)}
                />
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
