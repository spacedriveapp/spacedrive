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
export { useKeybind } from './hooks/useKeybind';
export type { KeybindHandler, UseKeybindOptions } from './hooks/useKeybind';
export { useKeybindScope } from './hooks/useKeybindScope';
export { useKeybindMeta, useKeybindDisplayString } from './hooks/useKeybindMeta';
export type { KeybindMeta } from './hooks/useKeybindMeta';

// Keybind system
export {
	// Registry
	KEYBINDS,
	explorerKeybinds,
	globalKeybinds,
	mediaViewerKeybinds,
	tagAssignerKeybinds,
	getKeybind,
	getAllKeybinds,
	getKeybindsByScope,
	isValidKeybindId,
	// Platform utilities
	getCurrentPlatform,
	getComboForPlatform,
	toDisplayString,
	// Types
	defineKeybind
} from './util/keybinds';
export type {
	KeybindId,
	Platform as KeybindPlatform,
	Modifier,
	Key,
	KeyCombo,
	PlatformKeyCombo,
	KeybindScope,
	KeybindDefinition
} from './util/keybinds';
