import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import type { SdPath, File } from "@sd/ts-client";
import { useExplorer } from "../../context";
import { useSelection } from "../../SelectionContext";
import { useNormalizedQuery } from "../../../../context";
import { useTabColumnSync } from "../../../TabManager";
import type { DirectorySortBy } from "@sd/ts-client";
import { Column } from "./Column";
import { useTypeaheadSearch } from "../../hooks/useTypeaheadSearch";
import { useVirtualListing } from "../../hooks/useVirtualListing";

export function ColumnView() {
	const { currentPath, navigateToPath, sortBy, viewSettings } = useExplorer();
	const { files: virtualFiles, isVirtualView } = useVirtualListing();
	const {
		selectedFiles,
		selectedFileIds,
		isSelected,
		selectFile,
		clearSelection,
	} = useSelection();
	const { savedColumnPaths, saveColumnPaths, activeTabId } =
		useTabColumnSync();
	const [columnStack, setColumnStack] = useState<SdPath[]>([]);

	// Store clearSelection in ref to avoid effect re-runs
	const clearSelectionRef = useRef(clearSelection);
	clearSelectionRef.current = clearSelection;

	// Track last processed tab and path for initialization
	const lastTabIdRef = useRef<string>("");
	const lastPathRef = useRef<string | null>(null);
	const justSwitchedTabRef = useRef<boolean>(false);

	// Typeahead search state
	const searchStringRef = useRef("");
	const searchTimeoutRef = useRef<NodeJS.Timeout | null>(null);

	// Initialize/restore columns when tab changes or external path change
	useEffect(() => {
		if (!currentPath) return;

		const currentPathStr =
			"Physical" in currentPath ? currentPath.Physical?.path : null;
		const isTabSwitch = lastTabIdRef.current !== activeTabId;
		const isPathChange = lastPathRef.current !== currentPathStr;

		// Update refs
		lastTabIdRef.current = activeTabId;
		lastPathRef.current = currentPathStr;

		// On tab switch: try to restore saved columns
		if (isTabSwitch) {
			justSwitchedTabRef.current = true;

			if (savedColumnPaths && savedColumnPaths.length > 0) {
				const firstSaved = savedColumnPaths[0];
				const savedFirstPath =
					firstSaved && "Physical" in firstSaved
						? firstSaved.Physical?.path
						: null;

				// Restore if saved columns match current path
				if (savedFirstPath === currentPathStr) {
					setColumnStack(savedColumnPaths);
					return;
				}
			}

			// No saved columns or mismatch - initialize with current path
			setColumnStack([currentPath]);
			clearSelectionRef.current();
			return;
		}

		// On path change within same tab (external navigation like sidebar click)
		// Skip if we just switched tabs (handled above)
		if (isPathChange && !justSwitchedTabRef.current) {
			setColumnStack([currentPath]);
			clearSelectionRef.current();
		}

		justSwitchedTabRef.current = false;
	}, [activeTabId, currentPath, savedColumnPaths]);

	// Save column stack whenever it changes
	useEffect(() => {
		if (columnStack.length > 0) {
			saveColumnPaths(columnStack);
		}
	}, [columnStack, saveColumnPaths]);

	// Handle file selection - uses global selectFile and updates columns
	const handleSelectFile = useCallback(
		(
			file: File,
			columnIndex: number,
			files: File[],
			multi = false,
			range = false,
		) => {
			// Use global selectFile to update selection state
			selectFile(file, files, multi, range);

			// Only update columns for single selection (not multi/range)
			if (!multi && !range) {
				if (file.kind === "Directory") {
					// Truncate columns after current and add new one
					// DON'T call navigateToPath - columnStack manages internal navigation
					// This prevents ExplorerLayout from re-rendering on every column change
					setColumnStack((prev) => [
						...prev.slice(0, columnIndex + 1),
						file.sd_path,
					]);
				} else {
					// For files, just truncate columns after current
					setColumnStack((prev) => prev.slice(0, columnIndex + 1));
				}
			}
		},
		[selectFile],
	);

	const handleNavigate = useCallback(
		(path: SdPath) => {
			navigateToPath(path);
		},
		[navigateToPath],
	);

	// Find the active column (the one containing the first selected file)
	const activeColumnIndex = useMemo(() => {
		if (selectedFiles.length === 0) return columnStack.length - 1; // Default to last column

		const firstSelected = selectedFiles[0];
		const filePath = firstSelected.sd_path.Physical?.path;
		if (!filePath) return columnStack.length - 1;

		const fileParent = filePath.substring(0, filePath.lastIndexOf("/"));

		return columnStack.findIndex((path) => {
			const columnPath = path.Physical?.path;
			return columnPath === fileParent;
		});
	}, [selectedFiles, columnStack]);

	const activeColumnPath = columnStack[activeColumnIndex];

	// Query files for the active column (for keyboard navigation)
	const activeColumnQuery = useNormalizedQuery({
		wireMethod: "query:files.directory_listing",
		input: activeColumnPath
			? {
					path: activeColumnPath,
					limit: null,
					include_hidden: false,
					sort_by: sortBy as DirectorySortBy,
					folders_first: viewSettings.foldersFirst,
				}
			: null!,
		resourceType: "file",
		enabled: !!activeColumnPath,
		pathScope: activeColumnPath,
	});

	const activeColumnFiles = activeColumnQuery.data?.files || [];

	// Typeahead search for active column
	const typeahead = useTypeaheadSearch({
		files: activeColumnFiles,
		onMatch: (file) => {
			handleSelectFile(file, activeColumnIndex, activeColumnFiles);
		},
	});

	// Query the next column for right arrow navigation
	const nextColumnPath = columnStack[activeColumnIndex + 1];
	const nextColumnQuery = useNormalizedQuery({
		wireMethod: "query:files.directory_listing",
		input: nextColumnPath
			? {
					path: nextColumnPath,
					limit: null,
					include_hidden: false,
					sort_by: sortBy as DirectorySortBy,
					folders_first: viewSettings.foldersFirst,
				}
			: null!,
		resourceType: "file",
		enabled: !!nextColumnPath,
		pathScope: nextColumnPath,
	});

	const nextColumnFiles = nextColumnQuery.data?.files || [];

	// Keyboard navigation
	useEffect(() => {
		const handleKeyDown = (e: KeyboardEvent) => {
			// Handle arrow keys
			if (
				["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(
					e.key,
				)
			) {
				e.preventDefault();

				if (e.key === "ArrowUp" || e.key === "ArrowDown") {
					// Navigate within current column
					if (activeColumnFiles.length === 0) return;

					const currentIndex =
						selectedFiles.length > 0
							? activeColumnFiles.findIndex(
									(f) => f.id === selectedFiles[0].id,
								)
							: -1;

					const newIndex =
						e.key === "ArrowDown"
							? currentIndex < 0
								? 0
								: Math.min(
										currentIndex + 1,
										activeColumnFiles.length - 1,
									)
							: currentIndex < 0
								? 0
								: Math.max(currentIndex - 1, 0);

					if (
						newIndex !== currentIndex &&
						activeColumnFiles[newIndex]
					) {
						const newFile = activeColumnFiles[newIndex];
						handleSelectFile(
							newFile,
							activeColumnIndex,
							activeColumnFiles,
						);

						// Scroll to keep selection visible
						const element = document.querySelector(
							`[data-file-id="${newFile.id}"]`,
						);
						if (element) {
							element.scrollIntoView({
								block: "nearest",
								behavior: "smooth",
							});
						}
					}
				} else if (e.key === "ArrowLeft") {
					// Move to previous column
					if (activeColumnIndex > 0) {
						// Truncate columns and stay at previous column
						// DON'T call navigateToPath - columnStack manages internal navigation
						setColumnStack((prev) =>
							prev.slice(0, activeColumnIndex),
						);
						clearSelectionRef.current();
					}
				} else if (e.key === "ArrowRight") {
					// If selected file is a directory and there's a next column, move focus there
					const firstSelected = selectedFiles[0];
					if (
						firstSelected?.kind === "Directory" &&
						activeColumnIndex < columnStack.length - 1
					) {
						// Select first item in next column
						if (nextColumnFiles.length > 0) {
							const firstFile = nextColumnFiles[0];
							handleSelectFile(
								firstFile,
								activeColumnIndex + 1,
								nextColumnFiles,
							);

							// Scroll to keep selection visible
							setTimeout(() => {
								const element = document.querySelector(
									`[data-file-id="${firstFile.id}"]`,
								);
								if (element) {
									element.scrollIntoView({
										block: "nearest",
										behavior: "smooth",
									});
								}
							}, 0);
						}
					}
				}
				return;
			}

			// Typeahead search for active column
			typeahead.handleKey(e);
		};

		window.addEventListener("keydown", handleKeyDown);
		return () => {
			window.removeEventListener("keydown", handleKeyDown);
			typeahead.cleanup();
		};
	}, [
		activeColumnFiles,
		nextColumnFiles,
		selectedFiles,
		activeColumnIndex,
		columnStack,
		handleSelectFile,
		typeahead,
	]);

	if (!currentPath && !isVirtualView) {
		return (
			<div className="flex items-center justify-center h-full">
				<div className="text-ink-dull">No location selected</div>
			</div>
		);
	}

	// Virtual listings: Show virtual column + next column if directory selected
	if (isVirtualView && virtualFiles) {
		// Check if a directory is selected in the virtual view
		const selectedDirectory =
			selectedFiles.length === 1 &&
			selectedFiles[0].kind === "Directory" &&
			selectedFiles[0].sd_path
				? selectedFiles[0]
				: null;

		return (
			<div className="flex h-full overflow-x-auto bg-app">
				{/* Virtual column (locations/volumes) */}
				<Column
					key="virtual-column"
					path={null as any}
					isSelected={isSelected}
					selectedFileIds={selectedFileIds}
					onSelectFile={(file, files, multi, range) => {
						selectFile(file, files, multi, range);
					}}
					onNavigate={handleNavigate}
					nextColumnPath={selectedDirectory?.sd_path}
					columnIndex={0}
					isActive={!selectedDirectory}
					virtualFiles={virtualFiles}
				/>

				{/* Next column showing selected directory contents */}
				{selectedDirectory && (
					<Column
						key={`dir-${selectedDirectory.id}`}
						path={selectedDirectory.sd_path}
						isSelected={isSelected}
						selectedFileIds={selectedFileIds}
						onSelectFile={(file, files, multi, range) =>
							handleSelectFile(file, 1, files, multi, range)
						}
						onNavigate={handleNavigate}
						nextColumnPath={undefined}
						columnIndex={1}
						isActive={true}
					/>
				)}
			</div>
		);
	}

	// Compute which columns are active based on selection
	// This is stable unless selection changes
	const activeColumnPaths = useMemo(() => {
		if (selectedFiles.length === 0) return new Set<string>();

		const paths = new Set<string>();
		for (const file of selectedFiles) {
			const filePath = file.sd_path.Physical?.path;
			if (!filePath) continue;
			const fileParent = filePath.substring(0, filePath.lastIndexOf("/"));
			paths.add(fileParent);
		}
		return paths;
	}, [selectedFiles]);

	return (
		<div className="flex h-full overflow-x-auto bg-app">
			{columnStack.map((path, index) => {
				const columnPath = path.Physical?.path || "";
				// A column is active if it contains a selected file or is the last column with no selection
				const isActive =
					selectedFiles.length > 0
						? activeColumnPaths.has(columnPath)
						: index === columnStack.length - 1;

				return (
					<Column
						key={`${path.Physical?.device_slug}-${path.Physical?.path}-${index}`}
						path={path}
						isSelected={isSelected}
						selectedFileIds={selectedFileIds}
						onSelectFile={(file, files, multi, range) =>
							handleSelectFile(file, index, files, multi, range)
						}
						onNavigate={handleNavigate}
						nextColumnPath={columnStack[index + 1]}
						columnIndex={index}
						isActive={isActive}
					/>
				);
			})}
		</div>
	);
}
