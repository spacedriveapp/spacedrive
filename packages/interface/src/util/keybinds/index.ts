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
	isInputFocused,
} from './platform';

// Registry
export {
	KEYBINDS,
	explorerKeybinds,
	globalKeybinds,
	mediaViewerKeybinds,
	quickPreviewKeybinds,
	getKeybind,
	getAllKeybinds,
	getKeybindsByScope,
} from './registry';

export type { KeybindId } from './registry';

// Listener
export {
	getWebListener,
	resetWebListener,
} from './listener';

export type { KeybindHandler } from './listener';
