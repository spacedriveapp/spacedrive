import { create } from "zustand";
import type { SdPath } from "@sd/ts-client";

export interface ClipboardState {
	operation: "copy" | "cut" | null;
	files: SdPath[];
	sourcePath: SdPath | null;
}

interface ClipboardStore extends ClipboardState {
	setClipboard: (
		operation: "copy" | "cut",
		files: SdPath[],
		sourcePath: SdPath | null,
	) => void;
	clearClipboard: () => void;
	hasClipboard: () => boolean;
}

export const useClipboardStore = create<ClipboardStore>((set, get) => ({
	operation: null,
	files: [],
	sourcePath: null,

	setClipboard: (operation, files, sourcePath) => {
		set({ operation, files, sourcePath });
		console.groupCollapsed(
			`[Clipboard] ${operation === "copy" ? "Copied" : "Cut"} ${files.length} file${files.length === 1 ? "" : "s"}`,
		);
		console.log("Operation:", operation);
		console.log("Source path:", sourcePath);
		console.log("Files (SdPath objects):");
		files.forEach((file, index) => {
			console.log(`  [${index}]:`, JSON.stringify(file, null, 2));
		});
		console.groupEnd();
	},

	clearClipboard: () => {
		const state = get();
		console.log(
			`[Clipboard] Cleared (had ${state.files.length} file${state.files.length === 1 ? "" : "s"})`,
		);
		set({ operation: null, files: [], sourcePath: null });
	},

	hasClipboard: () => {
		const state = get();
		return state.operation !== null && state.files.length > 0;
	},
}));

/**
 * Hook to access clipboard state and operations
 */
export function useClipboard() {
	const store = useClipboardStore();

	return {
		operation: store.operation,
		files: store.files,
		sourcePath: store.sourcePath,
		setClipboard: store.setClipboard,
		clearClipboard: store.clearClipboard,
		hasClipboard: store.hasClipboard,

		// Helper to copy files
		copyFiles: (files: SdPath[], sourcePath: SdPath | null = null) => {
			store.setClipboard("copy", files, sourcePath);
		},

		// Helper to cut files
		cutFiles: (files: SdPath[], sourcePath: SdPath | null = null) => {
			store.setClipboard("cut", files, sourcePath);
		},
	};
}
