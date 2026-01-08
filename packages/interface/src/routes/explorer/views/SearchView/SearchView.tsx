import { CaretDown } from "@phosphor-icons/react";
import type { File } from "@sd/ts-client";
import { flexRender } from "@tanstack/react-table";
import { useVirtualizer } from "@tanstack/react-virtual";
import clsx from "clsx";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useNormalizedQuery } from "../../../../contexts/SpacedriveContext";
import { useExplorer } from "../../context";
import { useSelection } from "../../SelectionContext";
import { FileCard } from "../GridView/FileCard";
import { TableRow } from "../ListView/TableRow";
import {
  ROW_HEIGHT,
  TABLE_HEADER_HEIGHT,
  TABLE_PADDING_X,
  TABLE_PADDING_Y,
  useTable,
} from "../ListView/useTable";

export function SearchView() {
  const explorer = useExplorer();
  const {
    isSelected,
    focusedIndex,
    setFocusedIndex,
    selectedFiles,
    selectFile,
    clearSelection,
    setSelectedFiles,
  } = useSelection();

  if (explorer.mode.type !== "search") {
    return null;
  }

  const { query, scope } = explorer.mode;
  const { viewMode, viewSettings, sortBy, setSortBy, currentPath } = explorer;
  const { gridSize, gapSize } = viewSettings;

  const searchQuery = useNormalizedQuery({
    wireMethod: "query:search.files",
    input: {
      query,
      scope:
        scope === "folder" && currentPath
          ? { Path: { path: currentPath } }
          : "Library",
      filters: explorer.searchFilters || {},
      mode: "Normal",
      sort: {
        field: sortBy?.by || "Relevance",
        direction: sortBy?.direction === "Asc" ? "Asc" : "Desc",
      },
      pagination: {
        limit: 1000,
        offset: 0,
      },
    },
    resourceType: "file",
    pathScope: scope === "folder" ? currentPath : undefined,
    enabled: query.length >= 2,
    debug: false,
  });

  const files = (searchQuery.data as any)?.files || [];

  useEffect(() => {
    explorer.setCurrentFiles(files);
  }, [searchQuery.data, explorer.setCurrentFiles]);

  if (query.length < 2) {
    return (
      <div className="flex h-full flex-col items-center justify-center p-8 text-center">
        <p className="text-ink-dull text-sm">
          Type at least 2 characters to search
        </p>
      </div>
    );
  }

  if (searchQuery.isLoading) {
    return (
      <div className="flex h-full flex-col items-center justify-center p-8 text-center">
        <p className="text-ink-dull text-sm">Searching...</p>
      </div>
    );
  }

  if (files.length === 0) {
    return (
      <div className="flex h-full flex-col items-center justify-center p-8 text-center">
        <p className="mb-2 text-ink-dull">No results found</p>
        <p className="text-ink-faint text-sm">
          Try a different search term or adjust your filters
        </p>
      </div>
    );
  }

  if (viewMode === "grid") {
    return <SearchGridView files={files} />;
  }

  if (viewMode === "list") {
    return <SearchListView files={files} />;
  }

  return (
    <div className="flex h-full flex-col items-center justify-center p-8 text-center">
      <p className="text-ink-dull text-sm">
        Search results in {viewMode} view coming soon
      </p>
    </div>
  );
}

function SearchGridView({ files }: { files: File[] }) {
  const explorer = useExplorer();
  const {
    isSelected,
    focusedIndex,
    setFocusedIndex,
    selectFile,
    selectedFiles,
  } = useSelection();
  const { gridSize, gapSize } = explorer.viewSettings;

  const containerRef = useRef<HTMLDivElement>(null);
  const [containerWidth, setContainerWidth] = useState(0);

  useLayoutEffect(() => {
    const updateWidth = () => {
      if (containerRef.current) {
        setContainerWidth(containerRef.current.offsetWidth);
      }
    };
    updateWidth();
    window.addEventListener("resize", updateWidth);
    return () => window.removeEventListener("resize", updateWidth);
  }, []);

  const padding = 24;
  const itemWidth = gridSize;
  const itemHeight = gridSize + 40;
  const columnsCount = Math.max(
    1,
    Math.floor((containerWidth - padding * 2 + gapSize) / (itemWidth + gapSize))
  );

  const virtualizer = useVirtualizer({
    count: Math.ceil(files.length / columnsCount),
    getScrollElement: () => containerRef.current,
    estimateSize: () => itemHeight + gapSize,
    overscan: 3,
  });

  return (
    <div className="h-full overflow-auto px-6 py-4" ref={containerRef}>
      <div
        style={{
          height: `${virtualizer.getTotalSize()}px`,
          position: "relative",
        }}
      >
        {virtualizer.getVirtualItems().map((virtualRow) => {
          const startIdx = virtualRow.index * columnsCount;
          const rowFiles = files.slice(startIdx, startIdx + columnsCount);

          return (
            <div
              key={virtualRow.key}
              style={{
                position: "absolute",
                top: 0,
                left: 0,
                width: "100%",
                transform: `translateY(${virtualRow.start}px)`,
              }}
            >
              <div
                style={{
                  display: "grid",
                  gridTemplateColumns: `repeat(${columnsCount}, ${itemWidth}px)`,
                  gap: `${gapSize}px`,
                }}
              >
                {rowFiles.map((file, colIndex) => {
                  const fileIndex = startIdx + colIndex;
                  return (
                    <FileCard
                      allFiles={files}
                      file={file}
                      fileIndex={fileIndex}
                      focused={focusedIndex === fileIndex}
                      key={file.id}
                      onFocus={() => setFocusedIndex(fileIndex)}
                      selected={isSelected(file.id)}
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

function SearchListView({ files }: { files: File[] }) {
  const explorer = useExplorer();
  const {
    focusedIndex,
    setFocusedIndex,
    isSelected,
    selectFile,
    selectedFiles,
  } = useSelection();
  const { sortBy, setSortBy } = explorer;

  const containerRef = useRef<HTMLDivElement>(null);
  const headerScrollRef = useRef<HTMLDivElement>(null);
  const bodyScrollRef = useRef<HTMLDivElement>(null);

  const table = useTable({
    files,
    sortBy,
    onSortChange: setSortBy,
  });

  const virtualizer = useVirtualizer({
    count: files.length,
    getScrollElement: () => bodyScrollRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 10,
  });

  const handleBodyScroll = () => {
    if (bodyScrollRef.current && headerScrollRef.current) {
      headerScrollRef.current.scrollLeft = bodyScrollRef.current.scrollLeft;
    }
  };

  return (
    <div className="flex h-full flex-col" ref={containerRef}>
      <div
        className="overflow-hidden"
        ref={headerScrollRef}
        style={{
          paddingLeft: TABLE_PADDING_X,
          paddingRight: TABLE_PADDING_X,
        }}
      >
        <div
          className="flex items-center border-sidebar-line/30 border-b"
          style={{
            width: table.getTotalSize(),
            height: TABLE_HEADER_HEIGHT,
          }}
        >
          {table.getHeaderGroups().map((headerGroup) =>
            headerGroup.headers.map((header) => (
              <div
                className={clsx(
                  "flex select-none items-center gap-1 px-3 font-medium text-sidebar-inkDull text-xs",
                  header.column.getCanSort() &&
                    "cursor-pointer hover:text-sidebar-ink"
                )}
                key={header.id}
                onClick={header.column.getToggleSortingHandler()}
                style={{ width: header.getSize() }}
              >
                {flexRender(
                  header.column.columnDef.header,
                  header.getContext()
                )}
                {header.column.getIsSorted() && (
                  <CaretDown
                    className={clsx(
                      "size-3 transition-transform",
                      header.column.getIsSorted() === "asc" && "rotate-180"
                    )}
                    weight="bold"
                  />
                )}
              </div>
            ))
          )}
        </div>
      </div>

      <div
        className="flex-1 overflow-auto"
        onScroll={handleBodyScroll}
        ref={bodyScrollRef}
        style={{
          paddingLeft: TABLE_PADDING_X,
          paddingRight: TABLE_PADDING_X,
          paddingTop: TABLE_PADDING_Y,
          paddingBottom: TABLE_PADDING_Y,
        }}
      >
        <div
          style={{
            height: `${virtualizer.getTotalSize()}px`,
            position: "relative",
          }}
        >
          {virtualizer.getVirtualItems().map((virtualRow) => {
            const file = files[virtualRow.index];
            const row = table.getRowModel().rows[virtualRow.index];

            return (
              <div
                key={virtualRow.key}
                style={{
                  position: "absolute",
                  top: 0,
                  left: 0,
                  width: "100%",
                  height: `${virtualRow.size}px`,
                  transform: `translateY(${virtualRow.start}px)`,
                }}
              >
                <TableRow
                  file={file}
                  focused={focusedIndex === virtualRow.index}
                  onFocus={() => setFocusedIndex(virtualRow.index)}
                  onSelect={(e) => selectFile(file, virtualRow.index, e)}
                  row={row}
                  selected={isSelected(file.id)}
                />
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
