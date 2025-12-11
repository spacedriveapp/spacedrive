// Platform detection
export type Platform = 'macos' | 'windows' | 'linux' | 'web';

// Modifier keys
export type Modifier = 'Cmd' | 'Ctrl' | 'Alt' | 'Shift';

// All supported keys (exhaustive)
export type Key =
	// Letters
	| 'a' | 'b' | 'c' | 'd' | 'e' | 'f' | 'g' | 'h' | 'i' | 'j' | 'k' | 'l' | 'm'
	| 'n' | 'o' | 'p' | 'q' | 'r' | 's' | 't' | 'u' | 'v' | 'w' | 'x' | 'y' | 'z'
	// Numbers
	| '0' | '1' | '2' | '3' | '4' | '5' | '6' | '7' | '8' | '9'
	// Special keys
	| 'Enter' | 'Escape' | 'Backspace' | 'Delete' | 'Tab' | 'Space'
	// Arrow keys
	| 'ArrowUp' | 'ArrowDown' | 'ArrowLeft' | 'ArrowRight'
	// Function keys
	| 'F1' | 'F2' | 'F3' | 'F4' | 'F5' | 'F6' | 'F7' | 'F8' | 'F9' | 'F10' | 'F11' | 'F12'
	// Punctuation and symbols
	| ',' | '.' | '/' | ';' | "'" | '[' | ']' | '\\' | '-' | '=' | '`';

// Key combination
export interface KeyCombo {
	modifiers: Modifier[];
	key: Key;
}

// Platform-specific overrides
export type PlatformKeyCombo = {
	[K in Platform]?: KeyCombo;
} & { default: KeyCombo };

// Keybind scope (context-aware)
export type KeybindScope =
	| 'global'
	| 'explorer'
	| 'settings'
	| 'mediaViewer'
	| 'tagAssigner'
	| 'quickPreview';

// Keybind definition
export interface KeybindDefinition {
	id: string;
	label: string;
	combo: KeyCombo | PlatformKeyCombo;
	scope: KeybindScope;
	preventDefault?: boolean;
}

// Helper to define keybinds with type safety
export function defineKeybind<T extends KeybindDefinition>(def: T): T {
	return def;
}

// Check if combo has platform-specific overrides
export function isPlatformKeyCombo(combo: KeyCombo | PlatformKeyCombo): combo is PlatformKeyCombo {
	return 'default' in combo;
}
