/**
 * Unified Keybind System
 *
 * This module provides a type-safe, platform-agnostic keybind abstraction.
 * It follows the same pattern as the context menu implementation:
 * - Definitions live in TypeScript (platform-agnostic)
 * - Tauri handles native shortcuts via window globals
 * - Web uses a JavaScript keydown listener as fallback
 *
 * @example
 * ```tsx
 * import { useKeybind, useKeybindScope, KEYBINDS } from '@sd/interface/keybinds';
 *
 * function ExplorerView() {
 *   // Activate explorer scope
 *   useKeybindScope('explorer');
 *
 *   // Register keybind handlers
 *   useKeybind('explorer.copy', () => {
 *     copySelectedFiles();
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

export { defineKeybind } from './types';

// Platform utilities
export {
	getCurrentPlatform,
	getComboForPlatform,
	normalizeModifiers,
	toTauriAccelerator,
	toDisplayString
} from './platform';

// Registry
export {
	KEYBINDS,
	explorerKeybinds,
	globalKeybinds,
	mediaViewerKeybinds,
	getKeybind,
	getAllKeybinds,
	getKeybindsByScope
} from './registry';

export type { KeybindId } from './registry';

// Listener
export { getWebListener, destroyWebListener } from './listener';

export type { KeybindHandler } from './listener';
