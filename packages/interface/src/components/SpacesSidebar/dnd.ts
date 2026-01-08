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
  console.log(
    "[DnD] setDragData called, data:",
    data,
    "listeners:",
    dragStateListeners.size
  );
  currentDragData = data;
  const isDragging = data !== null;

  // Always notify listeners immediately (sync)
  dragStateListeners.forEach((listener) => {
    console.log("[DnD] Calling listener with isDragging:", isDragging);
    listener(isDragging);
  });
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
