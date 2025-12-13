/**
 * Unified Keybind System
 *
 * A type-safe, platform-agnostic keybind system for Spacedrive.
 *
 * Key features:
 * - Single source of truth for all keybind definitions
 * - Type-safe keybind IDs with full autocomplete
 * - Platform abstraction (Cmd on macOS, Ctrl on Windows/Linux)
 * - Scope-aware keybinds (global vs component-specific)
 * - Context menu integration with automatic keybind display
 *
 * Usage:
 * ```tsx
 * import { useKeybind, useKeybindScope, KEYBINDS } from '~/util/keybinds';
 *
 * function ExplorerView() {
 *   // Activate the explorer scope
 *   useKeybindScope('explorer');
 *
 *   // Register a keybind handler
 *   useKeybind('explorer.copy', () => {
 *     console.log('Copy triggered!');
 *   });
 *
 *   return <div>...</div>;
 * }
 * ```
 */

// Types
export type {
	Platform,
	Modifier,
	Key,
	KeyCombo,
	PlatformKeyCombo,
	KeybindScope,
	KeybindDefinition
} from './types';

export { defineKeybind, isPlatformKeyCombo } from './types';

// Platform utilities
export {
	getCurrentPlatform,
	getComboForPlatform,
	normalizeModifiers,
	toTauriAccelerator,
	toDisplayString,
	eventMatchesCombo
} from './platform';

// Registry
export {
	KEYBINDS,
	explorerKeybinds,
	globalKeybinds,
	mediaViewerKeybinds,
	tagAssignerKeybinds,
	getKeybind,
	getAllKeybinds,
	getKeybindsByScope,
	isValidKeybindId
} from './registry';

export type { KeybindId } from './registry';

// Listener
export { getWebListener, resetWebListener } from './listener';
export type { KeybindHandler } from './listener';
