import { useRef, useMemo } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import clsx from "clsx";
import type { File, SdPath } from "@sd/ts-client";
import { useNormalizedQuery } from "../../../../context";
import { ColumnItem } from "./ColumnItem";
import { useExplorer } from "../../context";
import { useSelection } from "../../SelectionContext";
import { useContextMenu } from "../../../../hooks/useContextMenu";
import { Copy, Trash, Eye, FolderOpen } from "@phosphor-icons/react";
import { useLibraryMutation } from "../../../../context";

interface ColumnProps {
  path: SdPath;
  selectedFile: File | null;
  onSelectFile: (file: File) => void;
  onNavigate: (path: SdPath) => void;
  nextColumnPath?: SdPath;
  columnIndex: number;
  isActive: boolean;
}

export function Column({ path, selectedFile, onSelectFile, onNavigate, nextColumnPath, columnIndex, isActive }: ColumnProps) {
  const parentRef = useRef<HTMLDivElement>(null);
  const { viewSettings, sortBy } = useExplorer();
  const copyFiles = useLibraryMutation("files.copy");
  const deleteFiles = useLibraryMutation("files.delete");

  const directoryQuery = useNormalizedQuery({
    wireMethod: "query:files.directory_listing",
    input: {
      path: path,
      limit: null,
      include_hidden: false,
      sort_by: sortBy as any,
      folders_first: viewSettings.foldersFirst,
    },
    resourceType: "file",
    pathScope: path,
    // includeDescendants defaults to false for exact directory matching
  });

  const files = directoryQuery.data?.files || [];

  const rowVirtualizer = useVirtualizer({
    count: files.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 32,
    overscan: 10,
  });

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
        label: "Copy",
        onClick: async (file: File) => {
          window.__SPACEDRIVE__ = window.__SPACEDRIVE__ || {};
          window.__SPACEDRIVE__.clipboard = {
            operation: 'copy',
            files: [file.sd_path],
            sourcePath: path,
          };
        },
        keybind: "⌘C",
      },
      {
        icon: Copy,
        label: "Paste",
        onClick: async () => {
          const clipboard = window.__SPACEDRIVE__?.clipboard;
          if (!clipboard || !clipboard.files) return;

          try {
            await copyFiles.mutateAsync({
              sources: { paths: clipboard.files },
              destination: path,
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
        label: "Delete",
        onClick: async (file: File) => {
          if (confirm(`Delete "${file.name}"?`)) {
            try {
              await deleteFiles.mutateAsync({
                targets: { paths: [file.sd_path] },
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

            // Check if this file is selected
            const fileIsSelected = selectedFile?.id === file.id;

            // Check if this file is part of the navigation path
            const isInPath = nextColumnPath && file.sd_path.Physical && nextColumnPath.Physical
              ? file.sd_path.Physical.path === nextColumnPath.Physical.path &&
                file.sd_path.Physical.device_slug === nextColumnPath.Physical.device_slug
              : false;

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
                  selected={fileIsSelected || isInPath}
                  focused={false}
                  onClick={() => onSelectFile(file)}
                  onContextMenu={async (e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    onSelectFile(file);
                    await contextMenu.show(e);
                  }}
                />
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
