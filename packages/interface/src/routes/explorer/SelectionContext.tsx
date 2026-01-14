import {
	createContext,
	useContext,
	useState,
	useCallback,
	useMemo,
	useEffect,
	type ReactNode,
} from "react";
import { usePlatform } from "../../contexts/PlatformContext";
import type { File } from "@sd/ts-client";
import { useClipboard } from "../../hooks/useClipboard";
import { useLibraryMutation } from "../../contexts/SpacedriveContext";
import { useTabManager } from "../../components/TabManager";

interface SelectionContextValue {
	selectedFiles: File[];
	selectedFileIds: Set<string>;
	isSelected: (fileId: string) => boolean;
	setSelectedFiles: (files: File[]) => void;
	selectFile: (
		file: File,
		files: File[],
		multi?: boolean,
		range?: boolean,
	) => void;
	clearSelection: () => void;
	selectAll: (files: File[]) => void;
	focusedIndex: number;
	setFocusedIndex: (index: number) => void;
	moveFocus: (
		direction: "up" | "down" | "left" | "right",
		files: File[],
	) => void;
	// Rename state
	renamingFileId: string | null;
	startRename: (fileId: string) => void;
	cancelRename: () => void;
	saveRename: (newName: string) => Promise<void>;
	isRenaming: boolean;
	// Restore selection from available files (called by views when files load)
	restoreSelectionFromFiles: (files: File[]) => void;
}

const SelectionContext = createContext<SelectionContextValue | null>(null);

interface SelectionProviderProps {
	children: ReactNode;
	isActiveTab?: boolean;
}

export function SelectionProvider({
	children,
	isActiveTab = true,
}: SelectionProviderProps) {
	const platform = usePlatform();
	const clipboard = useClipboard();
	const tabManager = useTabManager();
	const { activeTabId, getSelectionIds, updateSelectionIds } = tabManager;
	const renameFile = useLibraryMutation("files.rename");

	// Local state for File objects (not serializable, can't be stored in TabManager)
	const [selectedFiles, setSelectedFilesInternal] = useState<File[]>([]);
	const [focusedIndex, setFocusedIndex] = useState(-1);
	const [lastSelectedIndex, setLastSelectedIndex] = useState(-1);
	const [renamingFileId, setRenamingFileId] = useState<string | null>(null);

	// Track the stored IDs for the active tab (separate from File objects)
	const storedIds = getSelectionIds(activeTabId);

	// Clear selection when activeTabId changes (we'll restore it when files load)
	useEffect(() => {
		setSelectedFilesInternal([]);
		setFocusedIndex(-1);
		setLastSelectedIndex(-1);
	}, [activeTabId]);

	// Wrapper for setSelectedFiles that syncs to TabManager
	// Supports both direct values and updater functions
	const setSelectedFiles = useCallback(
		(filesOrUpdater: File[] | ((prev: File[]) => File[])) => {
			setSelectedFilesInternal((prev) => {
				const nextFiles =
					typeof filesOrUpdater === "function"
						? filesOrUpdater(prev)
						: filesOrUpdater;

				// Sync to TabManager
				updateSelectionIds(
					activeTabId,
					nextFiles.map((f) => f.id),
				);

				return nextFiles;
			});
		},
		[activeTabId, updateSelectionIds],
	);

	// Sync selected file IDs to platform (for cross-window state sharing)
	// Only sync for the active tab to avoid conflicts
	useEffect(() => {
		if (!isActiveTab) return;

		const fileIds = selectedFiles.map((f) => f.id);

		if (platform.setSelectedFileIds) {
			platform.setSelectedFileIds(fileIds).catch((err) => {
				console.error(
					"Failed to sync selected files to platform:",
					err,
				);
			});
		}
	}, [selectedFiles, platform, isActiveTab]);

	// Update native menu items based on selection and clipboard state
	// Only update for active tab
	useEffect(() => {
		if (!isActiveTab) return;

		const hasSelection = selectedFiles.length > 0;
		const isSingleSelection = selectedFiles.length === 1;

		platform.updateMenuItems?.([
			// NOTE: copy/cut/paste are always enabled to support text input operations
			// They intelligently route to file ops or native clipboard based on focus
			{ id: "duplicate", enabled: hasSelection },
			{ id: "rename", enabled: isSingleSelection },
			{ id: "delete", enabled: hasSelection },
		]);
	}, [selectedFiles, clipboard, platform, isActiveTab]);

	const clearSelection = useCallback(() => {
		setSelectedFiles([]);
		setFocusedIndex(-1);
		setLastSelectedIndex(-1);
	}, [setSelectedFiles]);

	const selectAll = useCallback(
		(files: File[]) => {
			setSelectedFiles([...files]);
			setLastSelectedIndex(files.length - 1);
		},
		[setSelectedFiles],
	);

	const selectFile = useCallback(
		(file: File, files: File[], multi = false, range = false) => {
			const fileIndex = files.findIndex((f) => f.id === file.id);

			if (range) {
				setLastSelectedIndex((prevLastIndex) => {
					if (prevLastIndex !== -1) {
						const start = Math.min(prevLastIndex, fileIndex);
						const end = Math.max(prevLastIndex, fileIndex);
						const rangeFiles = files.slice(start, end + 1);

						setSelectedFiles((prev) => {
							// If there's already a multi-file selection, add the range (Finder behavior)
							if (prev.length > 1) {
								// Create a map for O(1) lookup
								const existingIds = new Set(
									prev.map((f) => f.id),
								);
								const combined = [...prev];

								// Add new range files that aren't already selected
								for (const rangeFile of rangeFiles) {
									if (!existingIds.has(rangeFile.id)) {
										combined.push(rangeFile);
									}
								}

								return combined;
							} else {
								// Single file or empty selection, replace with range
								return rangeFiles;
							}
						});
					}
					return fileIndex; // Update anchor to clicked file for next range
				});
				setFocusedIndex(fileIndex);
			} else if (multi) {
				setSelectedFiles((prev) => {
					const isSelected = prev.some((f) => f.id === file.id);
					if (isSelected) {
						return prev.filter((f) => f.id !== file.id);
					} else {
						return [...prev, file];
					}
				});
				setFocusedIndex(fileIndex);
				setLastSelectedIndex(fileIndex);
			} else {
				setSelectedFiles([file]);
				setFocusedIndex(fileIndex);
				setLastSelectedIndex(fileIndex);
			}
		},
		[setSelectedFiles],
	);

	const moveFocus = useCallback(
		(direction: "up" | "down" | "left" | "right", files: File[]) => {
			if (files.length === 0) return;

			setFocusedIndex((currentFocusedIndex) => {
				let newIndex = currentFocusedIndex;

				if (direction === "up")
					newIndex = Math.max(0, currentFocusedIndex - 1);
				if (direction === "down")
					newIndex = Math.min(
						files.length - 1,
						currentFocusedIndex + 1,
					);
				if (direction === "left")
					newIndex = Math.max(0, currentFocusedIndex - 1);
				if (direction === "right")
					newIndex = Math.min(
						files.length - 1,
						currentFocusedIndex + 1,
					);

				if (newIndex !== currentFocusedIndex) {
					setSelectedFiles([files[newIndex]]);
					setLastSelectedIndex(newIndex);
				}

				return newIndex;
			});
		},
		[setSelectedFiles],
	);

	// Rename functions
	const startRename = useCallback((fileId: string) => {
		// Only allow rename when a single file is selected
		if (selectedFiles.length === 1) {
			setRenamingFileId(fileId);
		}
	}, [selectedFiles.length]);

	const cancelRename = useCallback(() => {
		setRenamingFileId(null);
	}, []);

	const saveRename = useCallback(async (newName: string) => {
		if (!renamingFileId) return;

		const file = selectedFiles.find(f => f.id === renamingFileId);
		if (!file) {
			setRenamingFileId(null);
			return;
		}

		// Don't submit if name is empty or unchanged
		const currentFullName = file.extension ? `${file.name}.${file.extension}` : file.name;
		if (!newName.trim() || newName === currentFullName) {
			setRenamingFileId(null);
			return;
		}

		try {
			await renameFile.mutateAsync({
				target: file.sd_path,
				new_name: newName,
			});
			setRenamingFileId(null);
		} catch (error) {
			// Keep in edit mode on error so user can retry
			console.error('Rename failed:', error);
			throw error;
		}
	}, [renamingFileId, selectedFiles, renameFile]);

	// Cancel rename when selection changes
	useEffect(() => {
		if (renamingFileId && !selectedFiles.some(f => f.id === renamingFileId)) {
			setRenamingFileId(null);
		}
	}, [selectedFiles, renamingFileId]);

	// Use stored IDs for selection checking (allows highlighting before File objects are restored)
	const selectedFileIds = useMemo(
		() => new Set(storedIds),
		[storedIds],
	);

	// Stable function for checking if a file is selected
	const isSelected = useCallback(
		(fileId: string) => selectedFileIds.has(fileId),
		[selectedFileIds],
	);

	// Restore File objects for selected IDs when files become available
	const restoreSelectionFromFiles = useCallback(
		(files: File[]) => {
			if (storedIds.length === 0) return;

			const fileMap = new Map(files.map((f) => [f.id, f]));
			const matchingFiles: File[] = [];

			for (const id of storedIds) {
				const file = fileMap.get(id);
				if (file) {
					matchingFiles.push(file);
				}
			}

			// Only update if we found matching files and they're different from current
			if (matchingFiles.length > 0) {
				setSelectedFilesInternal((prev) => {
					const prevIds = new Set(prev.map((f) => f.id));
					const newIds = new Set(matchingFiles.map((f) => f.id));

					// Skip update if selection already matches
					if (
						prevIds.size === newIds.size &&
						[...newIds].every((id) => prevIds.has(id))
					) {
						return prev;
					}

					return matchingFiles;
				});
			}
		},
		[storedIds],
	);

	const isRenaming = renamingFileId !== null;

	const value = useMemo(
		() => ({
			selectedFiles,
			selectedFileIds,
			isSelected,
			setSelectedFiles,
			selectFile,
			clearSelection,
			selectAll,
			focusedIndex,
			setFocusedIndex,
			moveFocus,
			// Rename state
			renamingFileId,
			startRename,
			cancelRename,
			saveRename,
			isRenaming,
			// Restore selection
			restoreSelectionFromFiles,
		}),
		[
			selectedFiles,
			selectedFileIds,
			isSelected,
			setSelectedFiles,
			selectFile,
			clearSelection,
			selectAll,
			focusedIndex,
			moveFocus,
			renamingFileId,
			startRename,
			cancelRename,
			saveRename,
			isRenaming,
			restoreSelectionFromFiles,
		],
	);

	return (
		<SelectionContext.Provider value={value}>
			{children}
		</SelectionContext.Provider>
	);
}

export function useSelection() {
	const context = useContext(SelectionContext);
	if (!context)
		throw new Error("useSelection must be used within SelectionProvider");
	return context;
}