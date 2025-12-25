import { useEffect, useCallback, useMemo, useRef } from "react";
import type { SdPath, File } from "@sd/ts-client";
import { useExplorer } from "../../context";
import { useSelection } from "../../SelectionContext";
import { useNormalizedQuery } from "../../../../context";
import type { DirectorySortBy } from "@sd/ts-client";
import { Column } from "./Column";
import { useTypeaheadSearch } from "../../hooks/useTypeaheadSearch";
import { useVirtualListing } from "../../hooks/useVirtualListing";
import { isVirtualFile } from "../../utils/virtualFiles";

/** Get path string from SdPath for comparison */
function getPathString(path: SdPath | null | undefined): string {
	if (!path) return "";
	if ("Physical" in path) return path.Physical?.path || "";
	return JSON.stringify(path);
}

export function ColumnView() {
	const {
		currentPath,
		navigateToPath,
		sortBy,
		viewSettings,
		columnStack,
		setColumnStack,
		activeTabId,
	} = useExplorer();
	const { files: virtualFiles, isVirtualView } = useVirtualListing();
	const {
		selectedFiles,
		selectedFileIds,
		isSelected,
		selectFile,
		clearSelection,
	} = useSelection();

	// Store clearSelection in ref to avoid effect re-runs
	const clearSelectionRef = useRef(clearSelection);
	clearSelectionRef.current = clearSelection;

	// Store setColumnStack in ref to ensure we always have latest version
	const setColumnStackRef = useRef(setColumnStack);
	setColumnStackRef.current = setColumnStack;

	// Track the last tab ID and last path to detect actual changes
	const lastActiveTabIdRef = useRef<string>(activeTabId);
	const lastCurrentPathRef = useRef<string>(getPathString(currentPath));

	// Get current root path string
	const currentRootPath = getPathString(currentPath);

	// Get first column's root path from TabManager's columnStack
	const savedStackRoot = useMemo(() => {
		if (columnStack.length === 0) return "";
		return getPathString(columnStack[0]);
	}, [columnStack]);

	// Initialization logic:
	// columnStack comes from TabManager (authoritative per-tab state)
	// We only modify it when:
	// 1. Empty AND we have a currentPath (initial load or new tab)
	// 2. User navigated to a different location (currentPath CHANGED)
	useEffect(() => {
		// Detect tab switch
		const isTabSwitch = lastActiveTabIdRef.current !== activeTabId;

		// Detect if currentPath actually changed (user navigated somewhere new)
		const currentPathChanged =
			lastCurrentPathRef.current !== currentRootPath;

		// Update refs
		if (isTabSwitch) {
			lastActiveTabIdRef.current = activeTabId;
		}
		lastCurrentPathRef.current = currentRootPath;

		// If tab switched, don't touch anything - columnStack from TabManager is correct
		if (isTabSwitch) {
			return;
		}

		// No path = nothing to do
		if (!currentPath) return;

		// Empty columns = initialize with current path
		if (columnStack.length === 0) {
			setColumnStackRef.current([currentPath]);
			clearSelectionRef.current();
			return;
		}

		// Only reset columns if the user actually navigated to a different path
		// (not just because we re-rendered with existing state)
		if (currentPathChanged && savedStackRoot !== currentRootPath) {
			setColumnStackRef.current([currentPath]);
			clearSelectionRef.current();
		}
	}, [
		activeTabId,
		currentPath,
		currentRootPath,
		columnStack.length,
		savedStackRoot,
	]);

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
					const newStack = [
						...columnStack.slice(0, columnIndex + 1),
						file.sd_path,
					];
					setColumnStack(newStack);
				} else {
					// For files, just truncate columns after current
					const newStack = columnStack.slice(0, columnIndex + 1);
					setColumnStack(newStack);
				}
			}
		},
		[selectFile, columnStack, setColumnStack],
	);

	const handleNavigate = useCallback(
		(path: SdPath) => {
			navigateToPath(path);
		},
		[navigateToPath],
	);

	// Find the active column (the one containing the first selected file)
	// In virtual view: -1 = virtual column, 0+ = columnStack indices
	// In regular view: 0+ = columnStack indices
	const activeColumnIndex = useMemo(() => {
		if (selectedFiles.length === 0) {
			// No selection: last column is active
			return isVirtualView ? columnStack.length - 1 : columnStack.length - 1;
		}

		const firstSelected = selectedFiles[0];

		// Check if selected file is from virtual column
		if (isVirtualView && isVirtualFile(firstSelected)) {
			return -1; // Virtual column
		}

		const filePath =
			"Physical" in firstSelected.sd_path
				? firstSelected.sd_path.Physical?.path
				: null;
		if (!filePath) return columnStack.length - 1;

		const fileParent = filePath.substring(0, filePath.lastIndexOf("/"));

		return columnStack.findIndex((path) => {
			const columnPath = "Physical" in path ? path.Physical?.path : null;
			return columnPath === fileParent;
		});
	}, [selectedFiles, columnStack, isVirtualView]);

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
		enabled: !!activeColumnPath && activeColumnIndex >= 0,
		pathScope: activeColumnPath,
	});

	// Use virtual files if we're in the virtual column, otherwise use query data
	const activeColumnFiles =
		isVirtualView && activeColumnIndex === -1
			? virtualFiles || []
			: ((activeColumnQuery.data as any)?.files || []);

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

	const nextColumnFiles = (nextColumnQuery.data as any)?.files || [];

	// Keyboard navigation
	useEffect(() => {
		const handleKeyDown = (e: KeyboardEvent) => {
			if (
				["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(
					e.key,
				)
			) {
				e.preventDefault();

				if (e.key === "ArrowUp" || e.key === "ArrowDown") {
					if (activeColumnFiles.length === 0) return;

					const currentIndex =
						selectedFiles.length > 0
							? activeColumnFiles.findIndex(
									(f: File) => f.id === selectedFiles[0].id,
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
					// In virtual view, special handling for moving back to virtual column
					if (isVirtualView && activeColumnIndex === 0) {
						// We're in the first real column, go back to virtual column
						setColumnStack([]);
						clearSelectionRef.current();
					} else if (activeColumnIndex > 0) {
						// Regular navigation: truncate columnStack
						setColumnStack(columnStack.slice(0, activeColumnIndex));
						clearSelectionRef.current();
					}
				} else if (e.key === "ArrowRight") {
					const firstSelected = selectedFiles[0];

					// In virtual view, special handling for moving from virtual to first real column
					if (isVirtualView && activeColumnIndex === -1) {
						// We're in virtual column, navigate to first file in first real column
						if (columnStack.length > 0 && nextColumnFiles.length > 0) {
							const firstFile = nextColumnFiles[0];
							handleSelectFile(firstFile, 0, nextColumnFiles);

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
					} else if (
						firstSelected?.kind === "Directory" &&
						activeColumnIndex < columnStack.length - 1
					) {
						// Regular navigation: move to next column
						if (nextColumnFiles.length > 0) {
							const firstFile = nextColumnFiles[0];
							handleSelectFile(
								firstFile,
								activeColumnIndex + 1,
								nextColumnFiles,
							);

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
		setColumnStack,
		handleSelectFile,
		typeahead,
		isVirtualView,
	]);

	// Compute which columns are active based on selection
	// MUST be before any conditional returns to maintain hook order
	const activeColumnPaths = useMemo(() => {
		if (selectedFiles.length === 0) return new Set<string>();

		const paths = new Set<string>();
		for (const file of selectedFiles) {
			const filePath =
				"Physical" in file.sd_path ? file.sd_path.Physical?.path : null;
			if (!filePath) continue;
			const fileParent = filePath.substring(0, filePath.lastIndexOf("/"));
			paths.add(fileParent);
		}
		return paths;
	}, [selectedFiles]);

	if (!currentPath && !isVirtualView) {
		return (
			<div className="flex items-center justify-center h-full">
				<div className="text-ink-dull">No location selected</div>
			</div>
		);
	}

	// Virtual listings: Show virtual column + subsequent navigation columns
	if (isVirtualView && virtualFiles) {
		// Render virtual column + all columns in the stack
		return (
			<div className="flex h-full overflow-x-auto bg-app">
				{/* First column: Virtual listing */}
				<Column
					key="virtual-column"
					path={null as any}
					isSelected={isSelected}
					selectedFileIds={selectedFileIds}
					onSelectFile={(file, files, multi, range) => {
						// Use global selectFile to update selection state
						selectFile(file, files, multi, range);

						// Only update columns for single selection (not multi/range)
						if (!multi && !range) {
							if (file.kind === "Directory" && file.sd_path) {
								// Start a new column stack with the selected directory
								setColumnStack([file.sd_path]);
							} else {
								// For files, clear the stack
								setColumnStack([]);
							}
						}
					}}
					onNavigate={handleNavigate}
					nextColumnPath={columnStack[0]}
					columnIndex={0}
					isActive={columnStack.length === 0}
					virtualFiles={virtualFiles}
				/>

				{/* Subsequent columns: Real directory navigation */}
				{columnStack.map((path, index) => {
					const columnPath =
						"Physical" in path ? path.Physical?.path || "" : "";
					const isActive =
						selectedFiles.length > 0
							? activeColumnPaths.has(columnPath)
							: index === columnStack.length - 1;

					const deviceSlug =
						"Physical" in path ? path.Physical?.device_slug : "unknown";
					const pathStr =
						"Physical" in path ? path.Physical?.path : "unknown";

					return (
						<Column
							key={`${deviceSlug}-${pathStr}-${index}`}
							path={path}
							isSelected={isSelected}
							selectedFileIds={selectedFileIds}
							onSelectFile={(file, files, multi, range) =>
								handleSelectFile(file, index, files, multi, range)
							}
							onNavigate={handleNavigate}
							nextColumnPath={columnStack[index + 1]}
							columnIndex={index + 1} // +1 because virtual column is at 0
							isActive={isActive}
						/>
					);
				})}
			</div>
		);
	}

	return (
		<div className="flex h-full overflow-x-auto bg-app">
			{columnStack.map((path, index) => {
				const columnPath =
					"Physical" in path ? path.Physical?.path || "" : "";
				const isActive =
					selectedFiles.length > 0
						? activeColumnPaths.has(columnPath)
						: index === columnStack.length - 1;

				const deviceSlug =
					"Physical" in path ? path.Physical?.device_slug : "unknown";
				const pathStr =
					"Physical" in path ? path.Physical?.path : "unknown";

				return (
					<Column
						key={`${deviceSlug}-${pathStr}-${index}`}
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
