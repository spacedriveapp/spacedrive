import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useExplorer } from "../../context";
import { useSelection } from "../../SelectionContext";
import { useNormalizedQuery } from "../../../../context";
import { FileCard } from "../GridView/FileCard";
import { TableRow } from "../ListView/TableRow";
import {
	useTable,
	ROW_HEIGHT,
	TABLE_PADDING_X,
	TABLE_PADDING_Y,
	TABLE_HEADER_HEIGHT,
} from "../ListView/useTable";
import { flexRender } from "@tanstack/react-table";
import { CaretDown } from "@phosphor-icons/react";
import clsx from "clsx";
import type { File } from "@sd/ts-client";

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

	const files = (searchQuery.data as any)?.results || [];

	useEffect(() => {
		explorer.setCurrentFiles(files);
	}, [searchQuery.data, explorer.setCurrentFiles]);

	if (query.length < 2) {
		return (
			<div className="flex flex-col items-center justify-center h-full p-8 text-center">
				<p className="text-ink-dull text-sm">
					Type at least 2 characters to search
				</p>
			</div>
		);
	}

	if (searchQuery.isLoading) {
		return (
			<div className="flex flex-col items-center justify-center h-full p-8 text-center">
				<p className="text-ink-dull text-sm">Searching...</p>
			</div>
		);
	}

	if (files.length === 0) {
		return (
			<div className="flex flex-col items-center justify-center h-full p-8 text-center">
				<p className="text-ink-dull mb-2">No results found</p>
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
		<div className="flex flex-col items-center justify-center h-full p-8 text-center">
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
		Math.floor(
			(containerWidth - padding * 2 + gapSize) / (itemWidth + gapSize),
		),
	);

	const virtualizer = useVirtualizer({
		count: Math.ceil(files.length / columnsCount),
		getScrollElement: () => containerRef.current,
		estimateSize: () => itemHeight + gapSize,
		overscan: 3,
	});

	return (
		<div ref={containerRef} className="h-full overflow-auto px-6 py-4">
			<div
				style={{
					height: `${virtualizer.getTotalSize()}px`,
					position: "relative",
				}}
			>
				{virtualizer.getVirtualItems().map((virtualRow) => {
					const startIdx = virtualRow.index * columnsCount;
					const rowFiles = files.slice(
						startIdx,
						startIdx + columnsCount,
					);

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
											key={file.id}
											file={file}
											fileIndex={fileIndex}
											allFiles={files}
											selected={isSelected(file.id)}
											focused={focusedIndex === fileIndex}
											selectedFiles={selectedFiles}
											selectFile={selectFile}
											onFocus={() =>
												setFocusedIndex(fileIndex)
											}
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
			headerScrollRef.current.scrollLeft =
				bodyScrollRef.current.scrollLeft;
		}
	};

	return (
		<div ref={containerRef} className="flex flex-col h-full">
			<div
				ref={headerScrollRef}
				className="overflow-hidden"
				style={{
					paddingLeft: TABLE_PADDING_X,
					paddingRight: TABLE_PADDING_X,
				}}
			>
				<div
					style={{
						width: table.getTotalSize(),
						height: TABLE_HEADER_HEIGHT,
					}}
					className="flex items-center border-b border-sidebar-line/30"
				>
					{table.getHeaderGroups().map((headerGroup) =>
						headerGroup.headers.map((header) => (
							<div
								key={header.id}
								style={{ width: header.getSize() }}
								className={clsx(
									"flex items-center gap-1 px-3 text-xs font-medium text-sidebar-inkDull select-none",
									header.column.getCanSort() &&
										"cursor-pointer hover:text-sidebar-ink",
								)}
								onClick={header.column.getToggleSortingHandler()}
							>
								{flexRender(
									header.column.columnDef.header,
									header.getContext(),
								)}
								{header.column.getIsSorted() && (
									<CaretDown
										className={clsx(
											"size-3 transition-transform",
											header.column.getIsSorted() ===
												"asc" && "rotate-180",
										)}
										weight="bold"
									/>
								)}
							</div>
						)),
					)}
				</div>
			</div>

			<div
				ref={bodyScrollRef}
				onScroll={handleBodyScroll}
				className="flex-1 overflow-auto"
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
									row={row}
									file={file}
									selected={isSelected(file.id)}
									focused={focusedIndex === virtualRow.index}
									onSelect={(e) =>
										selectFile(file, virtualRow.index, e)
									}
									onFocus={() =>
										setFocusedIndex(virtualRow.index)
									}
								/>
							</div>
						);
					})}
				</div>
			</div>
		</div>
	);
}
