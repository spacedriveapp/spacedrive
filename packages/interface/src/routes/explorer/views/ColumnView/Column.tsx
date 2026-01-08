import type { File, SdPath } from "@sd/ts-client";
import { useVirtualizer, type VirtualItem } from "@tanstack/react-virtual";
import clsx from "clsx";
import { memo, useCallback, useRef } from "react";
import { useNormalizedQuery } from "../../../../contexts/SpacedriveContext";
import { useExplorer } from "../../context";
import { useFileContextMenu } from "../../hooks/useFileContextMenu";
import { useSelection } from "../../SelectionContext";
import { ColumnItem } from "./ColumnItem";

/**
 * Memoized wrapper for ColumnItem to prevent re-renders when selection changes elsewhere.
 * Only re-renders when this specific item's `selected` state changes.
 */
const ColumnItemWrapper = memo(
  function ColumnItemWrapper({
    file,
    files,
    virtualRow,
    selected,
    selectedFiles,
    onSelectFile,
    onNavigate,
  }: {
    file: File;
    files: File[];
    virtualRow: VirtualItem;
    selected: boolean;
    selectedFiles: File[];
    onSelectFile: (
      file: File,
      files: File[],
      multi?: boolean,
      range?: boolean
    ) => void;
    onNavigate: (path: SdPath) => void;
  }) {
    const contextMenu = useFileContextMenu({
      file,
      selectedFiles,
      selected,
    });

    const handleClick = useCallback(
      (multi: boolean, range: boolean) => {
        onSelectFile(file, files, multi, range);
      },
      [file, files, onSelectFile]
    );

    const handleDoubleClick = useCallback(() => {
      if (file.kind === "Directory" && file.sd_path) {
        onNavigate(file.sd_path);
      }
    }, [file, onNavigate]);

    const handleContextMenu = useCallback(
      async (e: React.MouseEvent) => {
        e.preventDefault();
        e.stopPropagation();
        if (!selected) {
          onSelectFile(file, files, false, false);
        }
        await contextMenu.show(e);
      },
      [file, files, selected, onSelectFile, contextMenu]
    );

    return (
      <div
        style={{
          position: "absolute",
          top: 0,
          left: 0,
          width: "100%",
          height: `${virtualRow.size}px`,
          transform: `translateY(${virtualRow.start}px)`,
        }}
      >
        <ColumnItem
          file={file}
          focused={false}
          onClick={handleClick}
          onContextMenu={handleContextMenu}
          onDoubleClick={handleDoubleClick}
          selected={selected}
        />
      </div>
    );
  },
  (prev, next) => {
    // Only re-render if selection state or file changed
    if (prev.selected !== next.selected) return false;
    if (prev.file !== next.file) return false;
    if (prev.virtualRow.start !== next.virtualRow.start) return false;
    if (prev.virtualRow.size !== next.virtualRow.size) return false;
    // Ignore: files array, onSelectFile, contextMenu (passed through to handlers)
    return true;
  }
);

interface ColumnProps {
  path: SdPath | null;
  isSelected: (fileId: string) => boolean;
  selectedFileIds: Set<string>;
  onSelectFile: (
    file: File,
    files: File[],
    multi?: boolean,
    range?: boolean
  ) => void;
  onNavigate: (path: SdPath) => void;
  nextColumnPath?: SdPath;
  columnIndex: number;
  isActive: boolean;
  virtualFiles?: File[];
}

export const Column = memo(function Column({
  path,
  isSelected,
  selectedFileIds,
  onSelectFile,
  onNavigate,
  nextColumnPath,
  columnIndex,
  isActive,
  virtualFiles,
}: ColumnProps) {
  const parentRef = useRef<HTMLDivElement>(null);
  const { viewSettings, sortBy } = useExplorer();
  const { selectedFiles } = useSelection();

  const directoryQuery = useNormalizedQuery({
    wireMethod: "query:files.directory_listing",
    input: {
      path: path!,
      limit: null,
      include_hidden: false,
      sort_by: sortBy as any,
      folders_first: viewSettings.foldersFirst,
    },
    resourceType: "file",
    pathScope: path ?? undefined,
    enabled: !!path && !virtualFiles,
    // includeDescendants defaults to false for exact directory matching
  });

  const files = virtualFiles || (directoryQuery.data as any)?.files || [];

  const rowVirtualizer = useVirtualizer({
    count: files.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 32,
    overscan: 10,
  });

  // Only show loading state if we're not using virtual files and the query is actually loading
  if (!virtualFiles && directoryQuery.isLoading) {
    return (
      <div
        className="flex shrink-0 items-center justify-center border-app-line border-r"
        style={{ width: `${viewSettings.columnWidth}px` }}
      >
        <div className="text-ink-dull text-sm">Loading...</div>
      </div>
    );
  }

  return (
    <div
      className={clsx(
        "shrink-0 overflow-auto border-app-line border-r",
        isActive && "bg-app-box/30"
      )}
      ref={parentRef}
      style={{ width: `${viewSettings.columnWidth}px` }}
    >
      <div
        style={{
          height: `${rowVirtualizer.getTotalSize()}px`,
          width: "100%",
          position: "relative",
        }}
      >
        {rowVirtualizer.getVirtualItems().map((virtualRow) => {
          const file = files[virtualRow.index];

          // Check if this file is selected using O(1) lookup
          const fileIsSelected = isSelected(file.id);

          // Check if this file is part of the navigation path
          const isInPath =
            nextColumnPath && file.sd_path
              ? JSON.stringify(file.sd_path) === JSON.stringify(nextColumnPath)
              : false;

          return (
            <ColumnItemWrapper
              file={file}
              files={files}
              key={virtualRow.key}
              onNavigate={onNavigate}
              onSelectFile={onSelectFile}
              selected={fileIsSelected || isInPath}
              selectedFiles={selectedFiles}
              virtualRow={virtualRow}
            />
          );
        })}
      </div>
    </div>
  );
});
