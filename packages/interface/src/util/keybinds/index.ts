// Types
export type { KeyCombo, Modifier, Key, PlatformKeyCombo, KeybindScope, KeybindDefinition } from './types';
export type { Platform } from './types';
export { defineKeybind, isPlatformKeyCombo } from './types';

// Platform utilities
export {
	getCurrentPlatform,
	getComboForPlatform,
	normalizeModifiers,
	toTauriAccelerator,
	toDisplayString,
	matchesKeyCombo
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
	getKeybindsByScope
} from './registry';

// Web listener
export type { KeybindHandler } from './listener';
export { getWebListener, resetWebListener } from './listener';
