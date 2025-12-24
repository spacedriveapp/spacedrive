import { useVirtualizer } from "@tanstack/react-virtual";
import {
	useRef,
	useMemo,
	useState,
	useEffect,
	useLayoutEffect,
	useCallback,
} from "react";
import { useExplorer } from "../../context";
import { useSelection } from "../../SelectionContext";
import { useNormalizedQuery } from "../../../../context";
import type { File } from "@sd/ts-client";
import { MediaViewItem } from "./MediaViewItem";
import { DateHeader, DATE_HEADER_HEIGHT } from "./DateHeader";
import { formatDate, getItemDate, normalizeDateToMidnight } from "./utils";

export function MediaView() {
	const { currentPath, viewSettings, sortBy, setSortBy, setCurrentFiles } =
		useExplorer();
	const {
		selectedFiles,
		selectFile,
		focusedIndex,
		setFocusedIndex,
		setSelectedFiles,
		isSelected,
		selectedFileIds,
	} = useSelection();

	// Set default sort to "datetaken" when entering media view
	useEffect(() => {
		if (
			sortBy !== "datetaken" &&
			sortBy !== "modified" &&
			sortBy !== "created" &&
			sortBy !== "name" &&
			sortBy !== "size"
		) {
			setSortBy("datetaken");
		}
	}, [sortBy, setSortBy]);
	const { gridSize } = viewSettings;
	// Fixed 1px gap for Apple Photos style
	const gapSize = 1;

	// ALL HOOKS MUST BE CALLED BEFORE ANY CONDITIONAL RETURNS
	const parentRef = useRef<HTMLDivElement>(null);
	const [containerWidth, setContainerWidth] = useState(0);
	const [scrollOffset, setScrollOffset] = useState(0);

	// TODO: Preserve scroll position per tab using scrollPosition from context

	// Track when element is ready
	const [elementReady, setElementReady] = useState(false);

	// Track container width with ResizeObserver AND window resize
	useEffect(() => {
		if (!elementReady) return;

		const element = parentRef.current;
		if (!element) return;

		let rafId: number | null = null;

		const updateWidth = () => {
			if (rafId) return; // Debounce with requestAnimationFrame

			rafId = requestAnimationFrame(() => {
				rafId = null;
				const newWidth = element.offsetWidth;

				if (newWidth > 0) {
					setContainerWidth(newWidth);
				}
			});
		};

		// ResizeObserver for when the element itself resizes
		const resizeObserver = new ResizeObserver(() => {
			updateWidth();
		});

		// Window resize listener as fallback
		const handleWindowResize = () => {
			updateWidth();
		};

		resizeObserver.observe(element);
		window.addEventListener("resize", handleWindowResize);

		// Set initial width immediately
		const initialWidth = element.offsetWidth;
		setContainerWidth(initialWidth);

		return () => {
			if (rafId) cancelAnimationFrame(rafId);
			resizeObserver.disconnect();
			window.removeEventListener("resize", handleWindowResize);
		};
	}, [elementReady]);

	// Track scroll position
	useEffect(() => {
		const element = parentRef.current;
		if (!element) return;

		const handleScroll = () => {
			setScrollOffset(element.scrollTop);
		};

		element.addEventListener("scroll", handleScroll, { passive: true });
		return () => {
			element.removeEventListener("scroll", handleScroll);
		};
	}, []);

	// Query for all media files from current path with descendants
	const mediaQuery = useNormalizedQuery({
		wireMethod: "query:files.media_listing",
		input: currentPath
			? {
					path: currentPath,
					include_descendants: true,
					media_types: null,
					limit: 10000,
					sort_by: sortBy as any, // MediaSortBy is a subset of DirectorySortBy
				}
			: null!,
		resourceType: "file",
		pathScope: currentPath,
		includeDescendants: true, // Recursive - show all media in subdirectories
		enabled: !!currentPath,
		// No resourceFilter needed - the backend query already filters for media
	});

	// Access files from the query response (reversed for inverted scroll)
	const files = useMemo(() => {
		return [...(mediaQuery.data?.files || [])].reverse();
	}, [mediaQuery.data?.files]);

	// Update current files in explorer context for quick preview navigation
	useEffect(() => {
		setCurrentFiles(files);
	}, [files, setCurrentFiles]);

	// Check if element is ready when files load
	useEffect(() => {
		if (parentRef.current && !elementReady) {
			setElementReady(true);
		}
	}, [files, elementReady]);

	// Keyboard navigation for media view
	useEffect(() => {
		const handleKeyDown = (e: KeyboardEvent) => {
			if (
				!["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(
					e.key,
				)
			) {
				return;
			}
			if (files.length === 0) return;

			e.preventDefault();

			// Calculate columns based on container width
			const itemWidth = gridSize + gapSize;
			const cols =
				containerWidth > 0
					? Math.max(4, Math.floor(containerWidth / itemWidth))
					: 8;

			let newIndex = focusedIndex;

			if (e.key === "ArrowUp") {
				newIndex = Math.max(0, focusedIndex - cols);
			} else if (e.key === "ArrowDown") {
				newIndex = Math.min(files.length - 1, focusedIndex + cols);
			} else if (e.key === "ArrowLeft") {
				newIndex = Math.max(0, focusedIndex - 1);
			} else if (e.key === "ArrowRight") {
				newIndex = Math.min(files.length - 1, focusedIndex + 1);
			}

			if (newIndex !== focusedIndex && files[newIndex]) {
				setFocusedIndex(newIndex);
				setSelectedFiles([files[newIndex]]);

				// Scroll selected item into view
				const element = document.querySelector(
					`[data-file-id="${files[newIndex].id}"]`,
				);
				if (element) {
					element.scrollIntoView({
						block: "nearest",
						behavior: "smooth",
					});
				}
			}
		};

		window.addEventListener("keydown", handleKeyDown);
		return () => window.removeEventListener("keydown", handleKeyDown);
	}, [
		files,
		focusedIndex,
		gridSize,
		gapSize,
		containerWidth,
		setFocusedIndex,
		setSelectedFiles,
	]);

	// Calculate columns and actual item size to fill available space
	const { columns, actualItemSize } = useMemo(() => {
		if (!containerWidth) return { columns: 8, actualItemSize: gridSize };
		const itemWidth = gridSize + gapSize;
		const cols = Math.max(4, Math.floor(containerWidth / itemWidth));
		// Calculate actual size to perfectly fill the width
		const totalGapWidth = (cols - 1) * gapSize;
		const availableWidth = containerWidth - totalGapWidth;
		const itemSize = Math.floor(availableWidth / cols);
		return { columns: cols, actualItemSize: itemSize };
	}, [containerWidth, gridSize, gapSize]);

	// Calculate row count
	const rowCount = Math.ceil(files.length / columns);

	// Calculate overscan based on viewport height (3 pages worth for smoother scrolling)
	const overscanCount = useMemo(() => {
		if (!parentRef.current) return 10;
		const viewportHeight = parentRef.current.clientHeight;
		const rowHeight = actualItemSize + gapSize;
		const rowsPerPage = Math.ceil(viewportHeight / rowHeight);
		// 3 pages in each direction to reduce flickering
		return Math.max(10, rowsPerPage * 3);
	}, [actualItemSize, gapSize, containerWidth]);

	// Row virtualizer for vertical scrolling
	const rowVirtualizer = useVirtualizer({
		count: rowCount,
		getScrollElement: () => parentRef.current,
		estimateSize: () => actualItemSize + gapSize,
		overscan: overscanCount,
	});

	// Force remeasure synchronously when layout changes (before paint)
	useLayoutEffect(() => {
		rowVirtualizer.measure();
	}, [columns, gridSize, rowCount, rowVirtualizer]);

	// Scroll to bottom on mount (inverted scroll - show most recent first)
	useEffect(() => {
		if (rowCount > 0 && parentRef.current) {
			rowVirtualizer.scrollToIndex(rowCount - 1, {
				align: "end",
			});
		}
	}, [rowCount, rowVirtualizer]);

	const virtualRows = rowVirtualizer.getVirtualItems();

	// Calculate date range for visible items
	const dateRange = useMemo(() => {
		if (files.length === 0 || virtualRows.length === 0) return undefined;

		const viewportHeight = parentRef.current?.clientHeight ?? 0;

		// Find first and last visible rows
		let firstRowIndex: number | undefined;
		let lastRowIndex: number | undefined;

		for (let i = 0; i < virtualRows.length; i++) {
			const row = virtualRows[i];
			if (row.end >= scrollOffset) {
				firstRowIndex = row.index;
				break;
			}
		}

		for (let i = virtualRows.length - 1; i >= 0; i--) {
			const row = virtualRows[i];
			if (row.start <= scrollOffset + viewportHeight) {
				lastRowIndex = row.index;
				break;
			}
		}

		if (firstRowIndex === undefined || lastRowIndex === undefined)
			return undefined;

		// Convert row indices to item indices
		let firstItemIndex = firstRowIndex * columns;
		let lastItemIndex = Math.min(
			lastRowIndex * columns + columns,
			files.length - 1,
		);

		// Find first valid date
		let firstDate: Date | undefined;
		for (let i = firstItemIndex; i <= lastItemIndex; i++) {
			const file = files[i];
			if (file) {
				const dateStr = getItemDate(file);
				if (dateStr) {
					firstDate = normalizeDateToMidnight(dateStr);
					break;
				}
			}
		}

		// Find last valid date
		let lastDate: Date | undefined;
		for (let i = lastItemIndex; i >= firstItemIndex; i--) {
			const file = files[i];
			if (file) {
				const dateStr = getItemDate(file);
				if (dateStr) {
					lastDate = normalizeDateToMidnight(dateStr);
					break;
				}
			}
		}

		if (!firstDate && !lastDate) return undefined;
		if (firstDate && !lastDate) return formatDate(firstDate);
		if (!firstDate && lastDate) return formatDate(lastDate);

		if (firstDate && lastDate) {
			if (firstDate.getTime() === lastDate.getTime()) {
				return formatDate(firstDate);
			}
			return formatDate({ from: firstDate, to: lastDate });
		}

		return undefined;
	}, [files, virtualRows, columns, scrollOffset, parentRef]);

	// NOW we can do conditional returns after all hooks are called
	// Show loading state
	if (mediaQuery.isLoading) {
		return (
			<div className="flex items-center justify-center h-full text-ink-dull">
				Loading media...
			</div>
		);
	}

	// Show empty state
	if (!currentPath) {
		return (
			<div className="flex flex-col items-center justify-center h-full text-ink-dull gap-2">
				<div className="text-lg">No location selected</div>
				<div className="text-sm">
					Select a location from the sidebar to view media
				</div>
			</div>
		);
	}

	if (files.length === 0) {
		return (
			<div className="flex flex-col items-center justify-center h-full text-ink-dull gap-2">
				<div className="text-lg">No media files found</div>
				<div className="text-sm">
					No images or videos in this location
				</div>
			</div>
		);
	}

	return (
		<div
			ref={parentRef}
			className="absolute inset-0 overflow-auto"
			style={{
				contain: "strict",
			}}
		>
			{/* Sticky date header in top-left corner */}
			<div className="sticky top-3 left-3 z-20 pointer-events-none">
				<DateHeader date={dateRange} />
			</div>

			<div
				className="relative w-full"
				style={{
					height: `${rowVirtualizer.getTotalSize()}px`,
					willChange: "contents",
				}}
			>
				{virtualRows.map((virtualRow) => {
					// Calculate items in this row
					const startIndex = virtualRow.index * columns;
					const endIndex = Math.min(
						startIndex + columns,
						files.length,
					);
					const rowTop = virtualRow.start;

					// Render items directly without intermediate array
					return Array.from(
						{ length: endIndex - startIndex },
						(_, idx) => {
							const i = startIndex + idx;
							const file = files[i];
							if (!file) return null;

							const columnIndex = i % columns;
							const left =
								columnIndex * (actualItemSize + gapSize);

							return (
								<div
									key={file.id}
									tabIndex={-1}
									className="absolute outline-none focus:outline-none"
									style={{
										top: `${rowTop}px`,
										left: `${left}px`,
										width: `${actualItemSize}px`,
										height: `${actualItemSize}px`,
									}}
								>
									<MediaViewItem
										file={file}
										allFiles={files}
										selected={selectedFileIds.has(file.id)}
										focused={i === focusedIndex}
										onSelect={selectFile}
										size={actualItemSize}
									/>
								</div>
							);
						},
					);
				})}
			</div>
		</div>
	);
}
