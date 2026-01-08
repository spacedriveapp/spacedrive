// Platform-agnostic Spacedrive interface
// This package contains all UI components, routes, and logic that works across
// Tauri (desktop), Web, and potentially mobile platforms

// Import global styles
import "./styles.css";

export { ErrorBoundary } from "./components/ErrorBoundary";
export type { InspectorVariant } from "./components/Inspector/Inspector";
export { Inspector, PopoutInspector } from "./components/Inspector/Inspector";
export { JobsScreen } from "./components/JobManager";
export { PairingModal } from "./components/modals/PairingModal";
export { QuickPreview } from "./components/QuickPreview";
// Platform abstraction
export type { Platform } from "./contexts/PlatformContext";
export { PlatformProvider, usePlatform } from "./contexts/PlatformContext";
export {
  type ServerContextValue,
  ServerProvider,
  useServer,
} from "./contexts/ServerContext";
// Context
export { SpacedriveProvider } from "./contexts/SpacedriveContext";
export { LocationCacheDemo } from "./demo/LocationCacheDemo";
export type { ClipboardState } from "./hooks/useClipboard";
// Clipboard hook
export { useClipboard, useClipboardStore } from "./hooks/useClipboard";
export type {
  ContextMenuConfig,
  ContextMenuItem,
} from "./hooks/useContextMenu";
// Hooks
export { useContextMenu } from "./hooks/useContextMenu";
export type { KeybindHandler, UseKeybindOptions } from "./hooks/useKeybind";
// Keybind hooks
export { useKeybind } from "./hooks/useKeybind";
export type { KeybindMeta } from "./hooks/useKeybindMeta";
export {
  useKeybindDisplayString,
  useKeybindMeta,
} from "./hooks/useKeybindMeta";
export { isScopeActive, useKeybindScope } from "./hooks/useKeybindScope";
export { Overview } from "./routes/overview";
export { Settings } from "./routes/settings";
export { Shell } from "./Shell";
export { TopBarPortal, TopBarProvider, useTopBar } from "./TopBar";
export type {
  KeybindDefinition,
  KeybindId,
  KeybindScope,
  KeyCombo,
} from "./util/keybinds";
// Keybind utilities
export {
  getAllKeybinds,
  getComboForPlatform,
  getCurrentPlatform,
  getKeybind,
  getKeybindsByScope,
  KEYBINDS,
  toDisplayString,
} from "./util/keybinds";
export { DemoWindow } from "./windows/DemoWindow";
export { FloatingControls } from "./windows/FloatingControls";
export { Spacedrop } from "./windows/Spacedrop";
