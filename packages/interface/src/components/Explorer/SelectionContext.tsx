import {
	createContext,
	useContext,
	useState,
	useCallback,
	useMemo,
	useEffect,
	type ReactNode,
} from "react";
import { usePlatform } from "../../platform";
import type { File } from "@sd/ts-client";
import { useClipboard } from "../../hooks/useClipboard";
import { useLibraryMutation } from "../../context";

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
	const [selectedFiles, setSelectedFiles] = useState<File[]>([]);
	const [focusedIndex, setFocusedIndex] = useState(-1);
	const [lastSelectedIndex, setLastSelectedIndex] = useState(-1);
	const [renamingFileId, setRenamingFileId] = useState<string | null>(null);
	const renameFile = useLibraryMutation("files.rename");

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
	}, []);

	const selectAll = useCallback((files: File[]) => {
		setSelectedFiles([...files]);
		setLastSelectedIndex(files.length - 1);
	}, []);

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
		[],
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
		[],
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

	// Create a Set of selected file IDs for O(1) lookup
	const selectedFileIds = useMemo(
		() => new Set(selectedFiles.map((f) => f.id)),
		[selectedFiles],
	);

	// Stable function for checking if a file is selected
	const isSelected = useCallback(
		(fileId: string) => selectedFileIds.has(fileId),
		[selectedFileIds],
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
		}),
		[
			selectedFiles,
			selectedFileIds,
			isSelected,
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
