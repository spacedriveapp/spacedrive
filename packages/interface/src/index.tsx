// Platform-agnostic Spacedrive interface
// This package contains all UI components, routes, and logic that works across
// Tauri (desktop), Web, and potentially mobile platforms

// Import global styles
import "./styles.css";

export { Explorer } from "./Explorer";
export { DemoWindow } from "./DemoWindow";
export { ErrorBoundary } from "./ErrorBoundary";
export { FloatingControls } from "./FloatingControls";
export { LocationCacheDemo } from "./LocationCacheDemo";
export { Inspector, PopoutInspector } from "./Inspector";
export type { InspectorVariant } from "./Inspector";
export { QuickPreview } from "./components/QuickPreview";
export { JobsScreen } from "./components/JobManager";
export { Settings } from "./Settings";
export { Spacedrop } from "./Spacedrop";
export { PairingModal } from "./components/PairingModal";
export { TopBarProvider, TopBarPortal, useTopBar } from "./TopBar";
export { Overview } from "./routes/overview";

// Platform abstraction
export type { Platform } from "./platform";
export { PlatformProvider, usePlatform } from "./platform";

// Context
export { SpacedriveProvider } from "./context";
export {
	ServerProvider,
	useServer,
	type ServerContextValue,
} from "./ServerContext";

// Hooks
export { useContextMenu } from "./hooks/useContextMenu";
export type {
	ContextMenuItem,
	ContextMenuConfig,
} from "./hooks/useContextMenu";

// Keybind hooks
export { useKeybind } from "./hooks/useKeybind";
export type { KeybindHandler, UseKeybindOptions } from "./hooks/useKeybind";
export { useKeybindScope, isScopeActive } from "./hooks/useKeybindScope";
export { useKeybindMeta, useKeybindDisplayString } from "./hooks/useKeybindMeta";
export type { KeybindMeta } from "./hooks/useKeybindMeta";

// Clipboard hook
export { useClipboard, useClipboardStore } from "./hooks/useClipboard";
export type { ClipboardState } from "./hooks/useClipboard";

// Keybind utilities
export {
	KEYBINDS,
	getKeybind,
	getAllKeybinds,
	getKeybindsByScope,
	getCurrentPlatform,
	getComboForPlatform,
	toDisplayString,
} from "./util/keybinds";
export type {
	KeybindId,
	KeybindScope,
	KeybindDefinition,
	KeyCombo,
} from "./util/keybinds";
