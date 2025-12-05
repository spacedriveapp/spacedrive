import { create } from "zustand";

export type LayoutMode = "grid" | "list" | "media";
export type SortBy =
	| "name"
	| "size"
	| "date_created"
	| "date_modified"
	| "kind";
export type SortOrder = "asc" | "desc";

interface ExplorerStore {
	// View mode
	layoutMode: LayoutMode;
	setLayoutMode: (mode: LayoutMode) => void;

	// Grid configuration
	gridColumns: number;
	setGridColumns: (columns: number) => void;

	// Sorting
	sortBy: SortBy;
	sortOrder: SortOrder;
	setSortBy: (sort: SortBy) => void;
	setSortOrder: (order: SortOrder) => void;

	// Selection
	selectedItems: Set<string>;
	isSelectionMode: boolean;
	selectItem: (id: string) => void;
	deselectItem: (id: string) => void;
	toggleItem: (id: string) => void;
	clearSelection: () => void;
	setSelectionMode: (enabled: boolean) => void;

	// Current path
	currentPath: string;
	setCurrentPath: (path: string) => void;

	// Current location
	currentLocationId: string | null;
	setCurrentLocation: (id: string | null) => void;
}

export const useExplorerStore = create<ExplorerStore>((set, get) => ({
	// View mode
	layoutMode: "grid",
	setLayoutMode: (mode) => set({ layoutMode: mode }),

	// Grid configuration
	gridColumns: 3,
	setGridColumns: (columns) => set({ gridColumns: columns }),

	// Sorting
	sortBy: "name",
	sortOrder: "asc",
	setSortBy: (sort) => set({ sortBy: sort }),
	setSortOrder: (order) => set({ sortOrder: order }),

	// Selection
	selectedItems: new Set(),
	isSelectionMode: false,
	selectItem: (id) =>
		set((state) => ({
			selectedItems: new Set([...state.selectedItems, id]),
		})),
	deselectItem: (id) =>
		set((state) => {
			const newSet = new Set(state.selectedItems);
			newSet.delete(id);
			return { selectedItems: newSet };
		}),
	toggleItem: (id) =>
		set((state) => {
			const newSet = new Set(state.selectedItems);
			if (newSet.has(id)) {
				newSet.delete(id);
			} else {
				newSet.add(id);
			}
			return {
				selectedItems: newSet,
				isSelectionMode: newSet.size > 0,
			};
		}),
	clearSelection: () =>
		set({ selectedItems: new Set(), isSelectionMode: false }),
	setSelectionMode: (enabled) =>
		set({
			isSelectionMode: enabled,
			selectedItems: enabled ? get().selectedItems : new Set(),
		}),

	// Current path
	currentPath: "/",
	setCurrentPath: (path) => set({ currentPath: path }),

	// Current location
	currentLocationId: null,
	setCurrentLocation: (id) => set({ currentLocationId: id }),
}));
