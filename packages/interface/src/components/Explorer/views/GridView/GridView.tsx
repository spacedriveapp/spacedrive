import { useEffect, useRef, useState, useMemo } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useExplorer } from "../../context";
import { useSelection } from "../../SelectionContext";
import { useNormalizedQuery } from "../../../../context";
import { FileCard } from "./FileCard";
import type { DirectorySortBy, File } from "@sd/ts-client";
import { useVirtualListing } from "../../hooks/useVirtualListing";

const VIRTUALIZATION_THRESHOLD = 0; // Disabled - always virtualize

export function GridView() {
  const { currentPath, sortBy, viewSettings, setCurrentFiles } = useExplorer();
  const { isSelected, focusedIndex, selectedFiles, selectFile, clearSelection } = useSelection();
  const { gridSize, gapSize } = viewSettings;

  // Check for virtual listing first
  const { files: virtualFiles, isVirtualView } = useVirtualListing();

  const directoryQuery = useNormalizedQuery({
    wireMethod: "query:files.directory_listing",
    input: currentPath
      ? {
          path: currentPath,
          limit: null,
          include_hidden: false,
          sort_by: sortBy as DirectorySortBy,
          folders_first: viewSettings.foldersFirst,
        }
      : null!,
    resourceType: "file",
    enabled: !!currentPath && !isVirtualView,
    pathScope: currentPath ?? undefined,
  });

  const files = isVirtualView ? (virtualFiles || []) : (directoryQuery.data?.files || []);

  // Update current files in explorer context for quick preview navigation
  useEffect(() => {
    setCurrentFiles(files);
  }, [files, setCurrentFiles]);

  const handleContainerClick = (e: React.MouseEvent) => {
    if (e.target === e.currentTarget) {
      clearSelection();
    }
  };

  // Conditional virtualization - use simple grid for small directories
  const shouldVirtualize = files.length > VIRTUALIZATION_THRESHOLD;

  if (!shouldVirtualize) {
    return (
      <div
        className="grid p-3 min-h-full"
        style={{
          gridTemplateColumns: `repeat(auto-fill, minmax(${gridSize}px, 1fr))`,
          gridAutoRows: 'max-content',
          gap: `${gapSize}px`,
        }}
        onClick={handleContainerClick}
      >
        {files.map((file, index) => (
          <FileCard
            key={file.id}
            file={file}
            fileIndex={index}
            allFiles={files}
            selected={isSelected(file.id)}
            focused={index === focusedIndex}
            selectedFiles={selectedFiles}
            selectFile={selectFile}
          />
        ))}
      </div>
    );
  }

  return (
    <VirtualizedGrid
      files={files}
      gridSize={gridSize}
      gapSize={gapSize}
      isSelected={isSelected}
      focusedIndex={focusedIndex}
      selectedFiles={selectedFiles}
      selectFile={selectFile}
      onContainerClick={handleContainerClick}
    />
  );
}

interface VirtualizedGridProps {
  files: File[];
  gridSize: number;
  gapSize: number;
  isSelected: (id: string) => boolean;
  focusedIndex: number;
  selectedFiles: File[];
  selectFile: (file: File, files: File[], multi?: boolean, range?: boolean) => void;
  onContainerClick: (e: React.MouseEvent) => void;
}

function VirtualizedGrid({
  files,
  gridSize,
  gapSize,
  isSelected,
  focusedIndex,
  selectedFiles,
  selectFile,
  onContainerClick,
}: VirtualizedGridProps) {
  const parentRef = useRef<HTMLDivElement>(null);
  const [containerWidth, setContainerWidth] = useState(0);

  // Track container width with ResizeObserver
  useEffect(() => {
    const element = parentRef.current;
    if (!element) return;

    let rafId: number | null = null;

    const updateWidth = () => {
      if (rafId) return;

      rafId = requestAnimationFrame(() => {
        rafId = null;
        const newWidth = element.offsetWidth;

        if (newWidth > 0) {
          // Subtract padding (p-3 = 12px on each side)
          setContainerWidth(newWidth - 24);
        }
      });
    };

    const resizeObserver = new ResizeObserver(updateWidth);
    resizeObserver.observe(element);
    window.addEventListener("resize", updateWidth);

    // Set initial width
    updateWidth();

    return () => {
      if (rafId) cancelAnimationFrame(rafId);
      resizeObserver.disconnect();
      window.removeEventListener("resize", updateWidth);
    };
  }, []);

  // Calculate columns (mimic auto-fill behavior)
  const columns = useMemo(() => {
    if (!containerWidth) return 1;

    // Mimic repeat(auto-fill, minmax(gridSize, 1fr))
    const minItemWidth = gridSize;
    const totalGapWidth = gapSize;

    // Calculate how many items fit
    let cols = 1;
    while (true) {
      const totalGaps = (cols - 1) * gapSize;
      const requiredWidth = cols * minItemWidth + totalGaps;

      if (requiredWidth <= containerWidth) {
        cols++;
      } else {
        cols--;
        break;
      }
    }

    return Math.max(1, cols);
  }, [containerWidth, gridSize, gapSize]);

  const rowCount = Math.ceil(files.length / columns);
  const rowGap = 4; // Gap between rows

  // Row virtualizer
  const rowVirtualizer = useVirtualizer({
    count: rowCount,
    getScrollElement: () => parentRef.current,
    estimateSize: () => gridSize + gapSize + rowGap,
    overscan: 5,
  });

  const virtualRows = rowVirtualizer.getVirtualItems();

  return (
    <div
      ref={parentRef}
      className="h-full overflow-auto"
      onClick={onContainerClick}
    >
      <div
        className="relative"
        style={{
          height: `${rowVirtualizer.getTotalSize()}px`,
          paddingTop: '12px',
          paddingBottom: '12px',
          minHeight: '100%',
        }}
      >
        {virtualRows.map((virtualRow) => {
          const startIndex = virtualRow.index * columns;
          const endIndex = Math.min(startIndex + columns, files.length);
          const rowFiles = files.slice(startIndex, endIndex);

          return (
            <div
              key={virtualRow.key}
              className="absolute left-0 w-full px-3"
              style={{
                top: `${virtualRow.start}px`,
                height: `${gridSize + gapSize}px`,
              }}
            >
              {/* CSS Grid within row - preserves flex-to-fill */}
              <div
                className="grid h-full"
                style={{
                  gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))`,
                  gap: `${gapSize}px`,
                }}
              >
                {rowFiles.map((file, idx) => {
                  const fileIndex = startIndex + idx;
                  return (
                    <FileCard
                      key={file.id}
                      file={file}
                      fileIndex={fileIndex}
                      allFiles={files}
                      selected={isSelected(file.id)}
                      focused={fileIndex === focusedIndex}
                      selectedFiles={selectedFiles}
                      selectFile={selectFile}
                    />
                  );
                })}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
