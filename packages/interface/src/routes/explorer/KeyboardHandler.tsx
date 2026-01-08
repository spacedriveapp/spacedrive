import { useExplorerKeyboard } from "./hooks/useExplorerKeyboard";

/**
 * Invisible component that handles keyboard events
 * Rendered separately to avoid causing parent rerenders
 */
export function KeyboardHandler() {
  useExplorerKeyboard();
  return null;
}