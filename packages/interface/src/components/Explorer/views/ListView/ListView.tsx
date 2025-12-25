import { useCallback, useRef, useEffect, memo, useMemo } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { flexRender } from "@tanstack/react-table";
import { CaretDown } from "@phosphor-icons/react";
import clsx from "clsx";

import type { File, DirectorySortBy } from "@sd/ts-client";

import { useExplorer } from "../../context";
import { useSelection } from "../../SelectionContext";
import { useNormalizedQuery } from "../../../../context";
import { TableRow } from "./TableRow";
import {
	useTable,
	ROW_HEIGHT,
	TABLE_PADDING_X,
	TABLE_PADDING_Y,
	TABLE_HEADER_HEIGHT,
} from "./useTable";
import { useVirtualListing } from "../../hooks/useVirtualListing";
import { DragSelect } from "./DragSelect";
import { useEmptySpaceContextMenu } from "../../hooks/useEmptySpaceContextMenu";

export const ListView = memo(function ListView() {
	const { currentPath, sortBy, setSortBy, viewSettings, setCurrentFiles } =
		useExplorer();
	const {
		focusedIndex,
		setFocusedIndex,
		selectedFiles,
		selectedFileIds,
		isSelected,
		selectFile,
		moveFocus,
	} = useSelection();

	const containerRef = useRef<HTMLDivElement>(null);
	const headerScrollRef = useRef<HTMLDivElement>(null);
	const bodyScrollRef = useRef<HTMLDivElement>(null);
	const emptySpaceContextMenu = useEmptySpaceContextMenu();

	// TODO: Preserve scroll position per tab using scrollPosition from context

	// Check for virtual listing first
	const { files: virtualFiles, isVirtualView } = useVirtualListing();

	// Memoize query input to prevent unnecessary re-fetches
	const queryInput = useMemo(
		() =>
			currentPath
				? {
						path: currentPath,
						limit: null,
						include_hidden: false,
						sort_by: sortBy as DirectorySortBy,
						folders_first: viewSettings.foldersFirst,
					}
				: null!,
		[currentPath, sortBy, viewSettings.foldersFirst],
	);

	const pathScope = useMemo(() => currentPath ?? undefined, [currentPath]);

	const directoryQuery = useNormalizedQuery({
		wireMethod: "query:files.directory_listing",
		input: queryInput,
		resourceType: "file",
		enabled: !!currentPath && !isVirtualView,
		pathScope,
	});

	const files = isVirtualView ? (virtualFiles || []) : (directoryQuery.data?.files || []);
	const { table } = useTable(files);
	const { rows } = table.getRowModel();

	// Update current files in explorer context for quick preview navigation
	useEffect(() => {
		setCurrentFiles(files);
	}, [files, setCurrentFiles]);

	// Virtual row rendering - uses the container as scroll element
	const rowVirtualizer = useVirtualizer({
		count: rows.length,
		getScrollElement: useCallback(() => containerRef.current, []),
		estimateSize: useCallback(() => ROW_HEIGHT, []),
		paddingStart: TABLE_HEADER_HEIGHT + TABLE_PADDING_Y,
		paddingEnd: TABLE_PADDING_Y,
		overscan: 15,
	});

	const virtualRows = rowVirtualizer.getVirtualItems();

	// Sync horizontal scroll between header and body
	const handleBodyScroll = useCallback(() => {
		if (bodyScrollRef.current && headerScrollRef.current) {
			headerScrollRef.current.scrollLeft =
				bodyScrollRef.current.scrollLeft;
		}
	}, []);

	const handleContainerContextMenu = async (e: React.MouseEvent) => {
		if (e.target === e.currentTarget) {
			e.preventDefault();
			e.stopPropagation();
			await emptySpaceContextMenu.show(e);
		}
	};

	// Store values in refs to avoid effect re-runs
	const rowVirtualizerRef = useRef(rowVirtualizer);
	rowVirtualizerRef.current = rowVirtualizer;
	const filesRef = useRef(files);
	filesRef.current = files;

	// Keyboard navigation - stable effect, uses refs for changing values
	useEffect(() => {
		const handleKeyDown = (e: KeyboardEvent) => {
			if (e.key === "ArrowDown" || e.key === "ArrowUp") {
				e.preventDefault();
				const direction = e.key === "ArrowDown" ? "down" : "up";
				const currentFiles = filesRef.current;

				const currentIndex = focusedIndex >= 0 ? focusedIndex : 0;
				const newIndex =
					direction === "down"
						? Math.min(currentIndex + 1, currentFiles.length - 1)
						: Math.max(currentIndex - 1, 0);

				if (e.shiftKey) {
					// Range selection with shift
					if (newIndex !== focusedIndex && currentFiles[newIndex]) {
						selectFile(
							currentFiles[newIndex],
							currentFiles,
							false,
							true,
						);
						setFocusedIndex(newIndex);
					}
				} else {
					moveFocus(direction, currentFiles);
				}

				// Scroll to keep selection visible
				rowVirtualizerRef.current.scrollToIndex(newIndex, {
					align: "auto",
				});
			}
		};

		window.addEventListener("keydown", handleKeyDown);
		return () => window.removeEventListener("keydown", handleKeyDown);
	}, [focusedIndex, selectFile, setFocusedIndex, moveFocus]);

	// Column sorting handler
	const handleHeaderClick = useCallback(
		(columnId: string) => {
			const sortMap: Record<string, DirectorySortBy> = {
				name: "name",
				size: "size",
				modified: "modified",
				type: "type",
			};
			const newSort = sortMap[columnId];
			if (newSort) {
				setSortBy(newSort);
			}
		},
		[setSortBy],
	);

	// Calculate total width for table
	const headerGroups = table.getHeaderGroups();
	const totalWidth = table.getTotalSize() + TABLE_PADDING_X * 2;

	return (
		<div ref={containerRef} className="h-full overflow-auto" onContextMenu={handleContainerContextMenu}>
			<DragSelect files={files} scrollRef={containerRef}>
				{/* Sticky Header */}
			<div
				className="sticky top-0 z-10 border-b border-app-line bg-app/90 backdrop-blur-lg"
				style={{ height: TABLE_HEADER_HEIGHT }}
			>
				<div ref={headerScrollRef} className="overflow-hidden">
					<div
						className="flex"
						style={{
							width: totalWidth,
							paddingLeft: TABLE_PADDING_X,
							paddingRight: TABLE_PADDING_X,
						}}
					>
						{headerGroups.map((headerGroup) =>
							headerGroup.headers.map((header) => {
								const isSorted = sortBy === header.id;
								const canResize = header.column.getCanResize();

								return (
									<div
										key={header.id}
										className={clsx(
											"relative flex select-none items-center gap-1 px-2 py-2 text-xs font-medium",
											isSorted
												? "text-ink"
												: "text-ink-dull",
											"cursor-pointer hover:text-ink",
										)}
										style={{ width: header.getSize() }}
										onClick={() =>
											handleHeaderClick(header.id)
										}
									>
										<span className="truncate">
											{flexRender(
												header.column.columnDef.header,
												header.getContext(),
											)}
										</span>

										{isSorted && (
											<CaretDown className="size-3 flex-shrink-0 text-ink-faint" />
										)}

										{/* Resize handle */}
										{canResize && (
											<div
												onMouseDown={header.getResizeHandler()}
												onTouchStart={header.getResizeHandler()}
												onClick={(e) =>
													e.stopPropagation()
												}
												className={clsx(
													"absolute right-0 top-1/2 h-4 w-1 -translate-y-1/2 cursor-col-resize rounded-full",
													header.column.getIsResizing()
														? "bg-accent"
														: "bg-transparent hover:bg-ink-faint/50",
												)}
											/>
										)}
									</div>
								);
							}),
						)}
					</div>
				</div>
			</div>

			{/* Virtual List Body */}
			<div
				ref={bodyScrollRef}
				className="overflow-x-auto"
				onScroll={handleBodyScroll}
				style={{ pointerEvents: "auto" }}
			>
				<div
					className="relative"
					style={{
						height:
							rowVirtualizer.getTotalSize() - TABLE_HEADER_HEIGHT,
						width: totalWidth,
						pointerEvents: "auto",
					}}
				>
					<div
						className="absolute left-0 top-0 w-full"
						style={{
							transform: `translateY(${(virtualRows[0]?.start ?? 0) - TABLE_HEADER_HEIGHT - TABLE_PADDING_Y}px)`,
							pointerEvents: "auto",
						}}
					>
						{virtualRows.map((virtualRow) => {
							const row = rows[virtualRow.index];
							if (!row) return null;

							const file = row.original;
							// Use O(1) lookup instead of O(n) selectedFiles.some()
							const fileIsSelected = isSelected(file.id);
							const isFocused = focusedIndex === virtualRow.index;
							const previousRow = rows[virtualRow.index - 1];
							const nextRow = rows[virtualRow.index + 1];
							// Use O(1) Set lookup for adjacent selection detection
							const isPreviousSelected = previousRow
								? selectedFileIds.has(previousRow.original.id)
								: false;
							const isNextSelected = nextRow
								? selectedFileIds.has(nextRow.original.id)
								: false;

							return (
								<TableRow
									key={row.id}
									row={row}
									file={file}
									files={files}
									index={virtualRow.index}
									isSelected={fileIsSelected}
									isFocused={isFocused}
									isPreviousSelected={isPreviousSelected}
									isNextSelected={isNextSelected}
									measureRef={rowVirtualizer.measureElement}
									selectFile={selectFile}
								/>
							);
						})}
					</div>
				</div>
			</div>
			</DragSelect>
		</div>
	);
});
