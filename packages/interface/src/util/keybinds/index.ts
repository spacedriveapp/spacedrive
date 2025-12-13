/**
 * Unified Keybind System
 *
 * This module provides a type-safe, platform-agnostic keybind abstraction.
 * Keybinds are defined once and automatically work on both web (JavaScript)
 * and native (Tauri global shortcuts) platforms.
 *
 * ## Usage
 *
 * ### Basic keybind registration
 * ```tsx
 * import { useKeybind } from '~/hooks/useKeybind';
 *
 * function MyComponent() {
 *   useKeybind('explorer.copy', async () => {
 *     await copySelectedFiles();
 *   });
 * }
 * ```
 *
 * ### Keybind scopes
 * ```tsx
 * import { useKeybindScope } from '~/hooks/useKeybindScope';
 *
 * function ExplorerView() {
 *   // Activate explorer scope while component is mounted
 *   useKeybindScope('explorer');
 *   return <div>...</div>;
 * }
 * ```
 *
 * ### Context menu integration
 * ```tsx
 * import { useContextMenu } from '~/hooks/useContextMenu';
 *
 * const menu = useContextMenu({
 *   items: [
 *     {
 *       label: 'Copy',
 *       onClick: handleCopy,
 *       keybindId: 'explorer.copy', // Auto-resolves to platform display string
 *     },
 *   ],
 * });
 * ```
 *
 * ### Get keybind metadata
 * ```tsx
 * import { useKeybindMeta } from '~/hooks/useKeybindMeta';
 *
 * function KeybindHint() {
 *   const meta = useKeybindMeta('explorer.copy');
 *   return <span>{meta?.displayString}</span>; // "Cmd+C" or "Ctrl+C"
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
	KeybindDefinition,
} from './types';
export { defineKeybind, isPlatformKeyCombo } from './types';

// Platform utilities
export {
	getCurrentPlatform,
	getComboForPlatform,
	normalizeModifiers,
	toTauriAccelerator,
	toDisplayString,
	matchesKeyCombo,
} from './platform';

// Registry
export type { KeybindId } from './registry';
export {
	KEYBINDS,
	explorerKeybinds,
	globalKeybinds,
	mediaViewerKeybinds,
	getKeybind,
	getAllKeybinds,
	getKeybindsByScope,
	getAllKeybindIds,
} from './registry';

// Listener (internal, but exported for advanced use cases)
export type { KeybindHandler } from './listener';
export { getWebListener, destroyWebListener } from './listener';
