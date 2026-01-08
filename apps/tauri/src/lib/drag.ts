import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// Types matching Rust definitions (lowercase to match serde rename_all = "camelCase")
export type DragItemKind =
  | { type: "file"; path: string }
  | { type: "filePromise"; name: string; mimeType: string }
  | { type: "text"; content: string };

export interface DragItem {
  kind: DragItemKind;
  id: string;
}

export type DragOperation = "copy" | "move" | "link";

export interface DragConfig {
  items: DragItem[];
  overlayUrl: string;
  overlaySize: [number, number];
  allowedOperations: DragOperation[];
}

export interface DragSession {
  id: string;
  config: DragConfig;
  sourceWindow: string;
  startedAt: number;
}

export type DragResult =
  | { type: "Dropped"; operation: DragOperation; target?: string }
  | { type: "Cancelled" }
  | { type: "Failed"; error: string };

// Event types
export interface DragBeganEvent {
  sessionId: string;
  sourceWindow: string;
  items: DragItem[];
}

export interface DragMoveEvent {
  sessionId: string;
  x: number;
  y: number;
}

export interface DragWindowEvent {
  sessionId: string;
  windowLabel: string;
}

export interface DragEndEvent {
  sessionId: string;
  result: DragResult;
}

export interface DropEvent {
  windowLabel: string;
  items: DragItem[];
  position: [number, number];
}

// Tauri command wrappers
export async function beginDrag(
  config: DragConfig,
  sourceWindowLabel: string
): Promise<string> {
  return await invoke("begin_drag", { config, sourceWindowLabel });
}

export async function endDrag(
  sessionId: string,
  result: DragResult
): Promise<void> {
  return await invoke("end_drag", { sessionId, result });
}

export async function getDragSession(): Promise<DragSession | null> {
  return await invoke("get_drag_session");
}

// Event listeners
export async function onDragBegan(
  handler: (event: DragBeganEvent) => void
): Promise<UnlistenFn> {
  return await listen<DragBeganEvent>("drag:began", (e) => handler(e.payload));
}

export async function onDragMoved(
  handler: (event: DragMoveEvent) => void
): Promise<UnlistenFn> {
  return await listen<DragMoveEvent>("drag:moved", (e) => handler(e.payload));
}

export async function onDragEntered(
  handler: (event: DragWindowEvent) => void
): Promise<UnlistenFn> {
  return await listen<DragWindowEvent>("drag:entered", (e) =>
    handler(e.payload)
  );
}

export async function onDragLeft(
  handler: (event: DragWindowEvent) => void
): Promise<UnlistenFn> {
  return await listen<DragWindowEvent>("drag:left", (e) => handler(e.payload));
}

export async function onDragEnded(
  handler: (event: DragEndEvent) => void
): Promise<UnlistenFn> {
  return await listen<DragEndEvent>("drag:ended", (e) => handler(e.payload));
}
