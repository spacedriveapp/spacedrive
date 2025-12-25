import { useEffect, useLayoutEffect, useRef, useState, useMemo } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useExplorer } from "../../context";
import { useSelection } from "../../SelectionContext";
import { useNormalizedQuery } from "../../../../context";
import { FileCard } from "./FileCard";
import type { DirectorySortBy, File } from "@sd/ts-client";
import { useVirtualListing } from "../../hooks/useVirtualListing";
import { DragSelect } from "./DragSelect";
import { useEmptySpaceContextMenu } from "../../hooks/useEmptySpaceContextMenu";

const VIRTUALIZATION_THRESHOLD = 0; // Disabled - always virtualize

export function GridView() {
	const { currentPath, sortBy, viewSettings, setCurrentFiles } =
		useExplorer();
	const {
		isSelected,
		focusedIndex,
		setFocusedIndex,
		selectedFiles,
		selectFile,
		clearSelection,
		setSelectedFiles,
	} = useSelection();
	const { gridSize, gapSize } = viewSettings;
	const emptySpaceContextMenu = useEmptySpaceContextMenu();

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
		// debug: true,
	});

	const files = isVirtualView
		? virtualFiles || []
		: (directoryQuery.data as any)?.files || [];

	// Update current files in explorer context for quick preview navigation
	useEffect(() => {
		setCurrentFiles(files);
	}, [files, setCurrentFiles]);

	const handleContainerClick = (e: React.MouseEvent) => {
		if (e.target === e.currentTarget) {
			clearSelection();
		}
	};

	const handleContainerContextMenu = async (e: React.MouseEvent) => {
		if (e.target === e.currentTarget) {
			e.preventDefault();
			e.stopPropagation();
			await emptySpaceContextMenu.show(e);
		}
	};

	// Conditional virtualization - use simple grid for small directories
	const shouldVirtualize = files.length > VIRTUALIZATION_THRESHOLD;
	const gridContainerRef = useRef<HTMLDivElement>(null);

	if (!shouldVirtualize) {
		return (
			<div
				ref={gridContainerRef}
				className="h-full overflow-auto"
				onClick={handleContainerClick}
				onContextMenu={handleContainerContextMenu}
			>
				<DragSelect files={files} scrollRef={gridContainerRef}>
					<div
						className="grid p-3 min-h-full"
						style={{
							gridTemplateColumns: `repeat(auto-fill, minmax(${gridSize}px, 1fr))`,
							gridAutoRows: "max-content",
							gap: `${gapSize}px`,
						}}
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
				</DragSelect>
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
			setFocusedIndex={setFocusedIndex}
			selectedFiles={selectedFiles}
			selectFile={selectFile}
			setSelectedFiles={setSelectedFiles}
			onContainerClick={handleContainerClick}
			onContainerContextMenu={handleContainerContextMenu}
		/>
	);
}

interface VirtualizedGridProps {
	files: File[];
	gridSize: number;
	gapSize: number;
	isSelected: (id: string) => boolean;
	focusedIndex: number;
	setFocusedIndex: (index: number) => void;
	selectedFiles: File[];
	selectFile: (
		file: File,
		files: File[],
		multi?: boolean,
		range?: boolean,
	) => void;
	setSelectedFiles: (files: File[]) => void;
	onContainerClick: (e: React.MouseEvent) => void;
	onContainerContextMenu: (e: React.MouseEvent) => void;
}

function VirtualizedGrid({
	files,
	gridSize,
	gapSize,
	isSelected,
	focusedIndex,
	setFocusedIndex,
	selectedFiles,
	selectFile,
	setSelectedFiles,
	onContainerClick,
	onContainerContextMenu,
}: VirtualizedGridProps) {
	const parentRef = useRef<HTMLDivElement>(null);
	const [containerWidth, setContainerWidth] = useState<number | null>(null);
	const [isInitialized, setIsInitialized] = useState(false);

	// TODO: Preserve scroll position per tab using scrollPosition from context

	// Synchronous measurement before paint to prevent layout shift
	useLayoutEffect(() => {
		const element = parentRef.current;
		if (!element) return;

		const updateWidth = () => {
			const newWidth = element.offsetWidth;

			if (newWidth > 0) {
				setContainerWidth(newWidth - 24);
				setIsInitialized(true);
			}
		};

		const resizeObserver = new ResizeObserver(updateWidth);
		resizeObserver.observe(element);

		// Immediate measurement
		updateWidth();

		return () => {
			resizeObserver.disconnect();
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

	// Keyboard navigation with correct column count
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

			let newIndex = focusedIndex < 0 ? 0 : focusedIndex;

			if (e.key === "ArrowUp") {
				newIndex = Math.max(0, newIndex - columns);
			} else if (e.key === "ArrowDown") {
				newIndex = Math.min(files.length - 1, newIndex + columns);
			} else if (e.key === "ArrowLeft") {
				newIndex = Math.max(0, newIndex - 1);
			} else if (e.key === "ArrowRight") {
				newIndex = Math.min(files.length - 1, newIndex + 1);
			}

			if (newIndex !== focusedIndex && files[newIndex]) {
				setFocusedIndex(newIndex);
				setSelectedFiles([files[newIndex]]);

				// Scroll into view
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
	}, [files, focusedIndex, columns, setFocusedIndex, setSelectedFiles]);

	return (
		<div
			ref={parentRef}
			className="h-full overflow-auto"
			onClick={onContainerClick}
			onContextMenu={onContainerContextMenu}
		>
			<DragSelect files={files} scrollRef={parentRef}>
				<div
					className="relative"
					style={{
						height: `${rowVirtualizer.getTotalSize()}px`,
						paddingTop: "12px",
						paddingBottom: "12px",
						minHeight: "100%",
						opacity: isInitialized ? 1 : 0,
						transition: "opacity 0.1s",
					}}
				>
					{virtualRows.map((virtualRow) => {
						const startIndex = virtualRow.index * columns;
						const endIndex = Math.min(
							startIndex + columns,
							files.length,
						);
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
												focused={
													fileIndex === focusedIndex
												}
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
			</DragSelect>
		</div>
	);
}
