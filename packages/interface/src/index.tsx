// Platform-agnostic Spacedrive interface
// This package contains all UI components, routes, and logic that works across
// Tauri (desktop), Web, and potentially mobile platforms

// Import global styles
import './styles.css';

export { Explorer } from './Explorer';
export { DemoWindow } from './DemoWindow';
export { ErrorBoundary } from './ErrorBoundary';
export { FloatingControls } from './FloatingControls';
export { LocationCacheDemo } from './LocationCacheDemo';
export { Inspector, PopoutInspector } from './Inspector';
export type { InspectorVariant } from './Inspector';
export { QuickPreview } from './components/QuickPreview';
export { Settings } from './Settings';
export { Spacedrop } from './Spacedrop';
export { PairingModal } from './components/PairingModal';
export { TopBarProvider, TopBarPortal, useTopBar } from './TopBar';
export { Overview } from './routes/overview';

// Platform abstraction
export type { Platform } from './platform';
export { PlatformProvider, usePlatform } from './platform';

// Context
export { SpacedriveProvider } from './context';

// Hooks
export { useContextMenu } from './hooks/useContextMenu';
export type { ContextMenuItem, ContextMenuConfig } from './hooks/useContextMenu';

// Keybind hooks
export { useKeybind } from './hooks/useKeybind';
export type { KeybindHandler, UseKeybindOptions } from './hooks/useKeybind';
export { useKeybindScope } from './hooks/useKeybindScope';
export { useKeybindMeta, useKeybindDisplayString } from './hooks/useKeybindMeta';

// Keybind utilities (re-export from util/keybinds)
export {
	KEYBINDS,
	explorerKeybinds,
	globalKeybinds,
	mediaViewerKeybinds,
	getKeybind,
	getAllKeybinds,
	getKeybindsByScope,
	getCurrentPlatform,
	getComboForPlatform,
	normalizeModifiers,
	toTauriAccelerator,
	toDisplayString,
	defineKeybind
} from './util/keybinds';

export type {
	KeybindId,
	KeyCombo,
	PlatformKeyCombo,
	KeybindScope,
	KeybindDefinition,
	Modifier,
	Key,
	Platform as KeybindPlatform
} from './util/keybinds';
