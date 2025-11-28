import type { SdPath } from "@sd/ts-client";

// Data transferred during drag operations
export interface SidebarDragData {
	type: "explorer-file";
	sdPath: SdPath;
	name: string;
}

// Global state for tracking internal app drag data
let currentDragData: SidebarDragData | null = null;

// Listeners for drag state changes
type DragStateListener = (isDragging: boolean) => void;
const dragStateListeners = new Set<DragStateListener>();

export function setDragData(data: SidebarDragData | null) {
	const wasDragging = currentDragData !== null;
	console.log("[DnD] setDragData:", data, "was:", currentDragData);
	currentDragData = data;
	const isDragging = data !== null;

	if (wasDragging !== isDragging) {
		dragStateListeners.forEach(listener => listener(isDragging));
	}
}

export function getDragData(): SidebarDragData | null {
	return currentDragData;
}

export function clearDragData() {
	setDragData(null);
}

export function isDragging(): boolean {
	return currentDragData !== null;
}

export function subscribeToDragState(listener: DragStateListener): () => void {
	dragStateListeners.add(listener);
	return () => dragStateListeners.delete(listener);
}
