import { create } from 'zustand';
import { persist } from 'zustand/middleware';

interface DraggedItem {
	type: 'file' | 'layout-item' | 'layout-group';
	data: any;
}

interface SidebarStore {
	// Persisted state
	currentLayoutId: string | null;
	setCurrentLayout: (id: string | null) => void;

	// Ephemeral state
	collapsedGroups: Set<string>;
	toggleGroup: (groupId: string) => void;
	collapseAll: (groupIds: string[]) => void;
	expandAll: () => void;

	// Drag state
	draggedItem: DraggedItem | null;
	setDraggedItem: (item: DraggedItem | null) => void;
}

export const useSidebarStore = create<SidebarStore>()(
	persist(
		(set) => ({
			// Persisted
			currentLayoutId: null,
			setCurrentLayout: (id) => set({ currentLayoutId: id }),

			// Ephemeral
			collapsedGroups: new Set(),
			toggleGroup: (groupId) =>
				set((state) => {
					const newSet = new Set(state.collapsedGroups);
					if (newSet.has(groupId)) {
						newSet.delete(groupId);
					} else {
						newSet.add(groupId);
					}
					return { collapsedGroups: newSet };
				}),
			collapseAll: (groupIds) =>
				set({
					collapsedGroups: new Set(groupIds),
				}),
			expandAll: () => set({ collapsedGroups: new Set() }),

			// Drag
			draggedItem: null,
			setDraggedItem: (item) => set({ draggedItem: item }),
		}),
		{
			name: 'spacedrive-sidebar',
			partialize: (state) => ({
				currentLayoutId: state.currentLayoutId,
			}),
		}
	)
);
