import { useEffect } from "react";
import { useExplorer } from "../context";
import { useSelection } from "../SelectionContext";
import { useNormalizedQuery } from "../../../context";
import type { DirectorySortBy } from "@sd/ts-client";
import { useTypeaheadSearch } from "./useTypeaheadSearch";
import { useKeybind } from "../../../hooks/useKeybind";
import { useKeybindScope } from "../../../hooks/useKeybindScope";
import { useClipboard } from "../../../hooks/useClipboard";
import { useFileOperationDialog } from "../../FileOperationModal";
import { isInputFocused } from "../../../util/keybinds/platform";

export function useExplorerKeyboard() {
	const {
		currentPath,
		sortBy,
		navigateToPath,
		viewMode,
		viewSettings,
		sidebarVisible,
		inspectorVisible,
		openQuickPreview,
		tagModeActive,
		setTagModeActive,
	} = useExplorer();
	const {
		selectedFiles,
		selectFile,
		selectAll,
		clearSelection,
		focusedIndex,
		setFocusedIndex,
		setSelectedFiles,
		startRename,
		isRenaming,
	} = useSelection();
	const clipboard = useClipboard();
	const openFileOperation = useFileOperationDialog();

	// Activate explorer keybind scope when this hook is active
	useKeybindScope("explorer");

	// Query files for keyboard operations
	const directoryQuery = useNormalizedQuery({
		wireMethod: "query:files.directory_listing",
		input: currentPath
			? {
					path: currentPath,
					limit: null,
					include_hidden: false,
					sort_by: sortBy as DirectorySortBy,
				}
			: null!,
		resourceType: "file",
		enabled: !!currentPath,
		pathScope: currentPath ?? undefined,
	});

	const files = (directoryQuery.data as any)?.files || [];

	// Typeahead search (disabled for column view - it handles its own)
	const typeahead = useTypeaheadSearch({
		files,
		onMatch: (file, index) => {
			setFocusedIndex(index);
			setSelectedFiles([file]);
		},
		enabled: viewMode !== "column",
	});

	// Copy: Store selected files in clipboard
	useKeybind(
		"explorer.copy",
		() => {
			if (selectedFiles.length === 0) return;
			const sdPaths = selectedFiles.map((f) => f.sd_path);
			clipboard.copyFiles(sdPaths, currentPath);
		},
		{ enabled: selectedFiles.length > 0 },
	);

	// Cut: Store selected files in clipboard with cut operation
	useKeybind(
		"explorer.cut",
		() => {
			if (selectedFiles.length === 0) return;
			const sdPaths = selectedFiles.map((f) => f.sd_path);
			clipboard.cutFiles(sdPaths, currentPath);
		},
		{ enabled: selectedFiles.length > 0 },
	);

	// Paste: Open file operation modal with clipboard contents
	useKeybind(
		"explorer.paste",
		() => {
			if (!clipboard.hasClipboard() || !currentPath) return;

			const operation = clipboard.operation === "cut" ? "move" : "copy";

			console.groupCollapsed(
				`[Clipboard] Pasting ${clipboard.files.length} file${clipboard.files.length === 1 ? "" : "s"} (${operation})`,
			);
			console.log("Operation:", operation);
			console.log("Destination:", currentPath);
			console.log("Source files (SdPath objects):");
			clipboard.files.forEach((file, index) => {
				console.log(`  [${index}]:`, JSON.stringify(file, null, 2));
			});
			console.groupEnd();

			openFileOperation({
				operation,
				sources: clipboard.files,
				destination: currentPath,
				onComplete: () => {
					// Clear clipboard after cut operation completes
					if (clipboard.operation === "cut") {
						console.log(
							"[Clipboard] Operation completed, clearing clipboard",
						);
						clipboard.clearClipboard();
					} else {
						console.log("[Clipboard] Copy operation completed");
					}
				},
			});
		},
		{ enabled: clipboard.hasClipboard() && !!currentPath },
	);

	// Rename: Enter key triggers rename mode for any selected file or directory
	useKeybind(
		"explorer.renameFile",
		() => {
			if (selectedFiles.length === 1 && !isRenaming) {
				startRename(selectedFiles[0].id);
			}
		},
		{ enabled: selectedFiles.length === 1 && !isRenaming },
	);

	// Tag mode: T key enters tag assignment mode
	useKeybind(
		"explorer.enterTagMode",
		() => {
			setTagModeActive(true);
		},
		{ enabled: !tagModeActive },
	);

	// Quick Preview: Spacebar opens quick preview
	useKeybind(
		"explorer.toggleQuickPreview",
		() => {
			if (selectedFiles.length === 1) {
				openQuickPreview(selectedFiles[0].id);
			}
		},
		{ enabled: selectedFiles.length === 1 },
	);

	useEffect(() => {
		const handleKeyDown = async (e: KeyboardEvent) => {
			// Skip all keyboard shortcuts if renaming or typing in an input
			if (isRenaming || isInputFocused()) return;

			// Arrow keys: Navigation
			if (
				["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(
					e.key,
				)
			) {
				// Skip views that handle their own keyboard navigation
				if (
					viewMode === "column" ||
					viewMode === "media" ||
					viewMode === "grid"
				) {
					return;
				}

				e.preventDefault();

				if (files.length === 0) return;

				let newIndex = focusedIndex;

				if (viewMode === "list") {
					// List view: only up/down
					if (e.key === "ArrowUp")
						newIndex = Math.max(0, focusedIndex - 1);
					if (e.key === "ArrowDown")
						newIndex = Math.min(files.length - 1, focusedIndex + 1);
				} else if (viewMode === "grid" || viewMode === "media") {
					// Grid/Media view: 2D navigation
					const containerWidth =
						window.innerWidth -
						(sidebarVisible ? 224 : 0) -
						(inspectorVisible ? 284 : 0) -
						48;
					const itemWidth =
						viewSettings.gridSize + viewSettings.gapSize;
					const columns = Math.floor(containerWidth / itemWidth);

					if (e.key === "ArrowUp")
						newIndex = Math.max(0, focusedIndex - columns);
					if (e.key === "ArrowDown")
						newIndex = Math.min(
							files.length - 1,
							focusedIndex + columns,
						);
					if (e.key === "ArrowLeft")
						newIndex = Math.max(0, focusedIndex - 1);
					if (e.key === "ArrowRight")
						newIndex = Math.min(files.length - 1, focusedIndex + 1);
				}

				if (newIndex !== focusedIndex) {
					setFocusedIndex(newIndex);
					setSelectedFiles([files[newIndex]]);
				}
				return;
			}

			// Cmd/Ctrl+A: Select all
			if ((e.metaKey || e.ctrlKey) && e.key === "a") {
				e.preventDefault();
				selectAll(files);
				return;
			}

			// Escape: Clear selection
			if (e.code === "Escape" && selectedFiles.length > 0) {
				clearSelection();
			}

			// Typeahead search (handled by hook, disabled for column view)
			typeahead.handleKey(e);
		};

		window.addEventListener("keydown", handleKeyDown);
		return () => {
			window.removeEventListener("keydown", handleKeyDown);
			typeahead.cleanup();
		};
	}, [
		selectedFiles,
		files,
		focusedIndex,
		viewMode,
		viewSettings,
		sidebarVisible,
		inspectorVisible,
		selectAll,
		clearSelection,
		navigateToPath,
		setFocusedIndex,
		setSelectedFiles,
		openQuickPreview,
		isRenaming,
		typeahead,
	]);
}
