/**
 * Unified Keybind System - Type Definitions
 *
 * Platform-agnostic type system for defining keyboard shortcuts.
 * Supports platform-specific overrides and scope-aware keybinds.
 */

/** Platform detection */
export type Platform = 'macos' | 'windows' | 'linux' | 'web';

/** Modifier keys */
export type Modifier = 'Cmd' | 'Ctrl' | 'Alt' | 'Shift';

/**
 * All supported keys (exhaustive union type)
 * Provides full autocomplete and type safety
 */
export type Key =
	// Letters
	| 'a'
	| 'b'
	| 'c'
	| 'd'
	| 'e'
	| 'f'
	| 'g'
	| 'h'
	| 'i'
	| 'j'
	| 'k'
	| 'l'
	| 'm'
	| 'n'
	| 'o'
	| 'p'
	| 'q'
	| 'r'
	| 's'
	| 't'
	| 'u'
	| 'v'
	| 'w'
	| 'x'
	| 'y'
	| 'z'
	// Numbers
	| '0'
	| '1'
	| '2'
	| '3'
	| '4'
	| '5'
	| '6'
	| '7'
	| '8'
	| '9'
	// Special keys
	| 'Enter'
	| 'Escape'
	| 'Backspace'
	| 'Delete'
	| 'Tab'
	| 'Space'
	// Arrow keys
	| 'ArrowUp'
	| 'ArrowDown'
	| 'ArrowLeft'
	| 'ArrowRight'
	// Function keys
	| 'F1'
	| 'F2'
	| 'F3'
	| 'F4'
	| 'F5'
	| 'F6'
	| 'F7'
	| 'F8'
	| 'F9'
	| 'F10'
	| 'F11'
	| 'F12'
	// Punctuation and symbols
	| ','
	| '.'
	| '/'
	| ';'
	| "'"
	| '['
	| ']'
	| '\\'
	| '-'
	| '='
	| '`';

/** Key combination - modifiers + a key */
export interface KeyCombo {
	modifiers: Modifier[];
	key: Key;
}

/**
 * Platform-specific key combo overrides
 * Must include a default fallback
 */
export type PlatformKeyCombo = {
	[K in Platform]?: KeyCombo;
} & { default: KeyCombo };

/**
 * Keybind scope - determines when keybinds are active
 * - global: Always active
 * - explorer: Only when Explorer component is mounted
 * - mediaViewer: Only when media viewer is open
 * - settings: Only in settings pages
 * - tagAssigner: Only when tag assigner is open
 */
export type KeybindScope = 'global' | 'explorer' | 'settings' | 'mediaViewer' | 'tagAssigner';

/**
 * Keybind definition
 * Single source of truth for a keyboard shortcut
 */
export interface KeybindDefinition {
	/** Unique identifier in format "scope.action" */
	id: string;
	/** Human-readable label for display */
	label: string;
	/** Key combination (simple or platform-specific) */
	combo: KeyCombo | PlatformKeyCombo;
	/** Scope in which this keybind is active */
	scope: KeybindScope;
	/** Whether to prevent default browser behavior */
	preventDefault?: boolean;
}

/**
 * Helper to define keybinds with type safety
 * Use this to get full autocomplete when defining keybinds
 */
export function defineKeybind<T extends KeybindDefinition>(def: T): T {
	return def;
}

/**
 * Type guard to check if a combo is platform-specific
 */
export function isPlatformKeyCombo(
	combo: KeyCombo | PlatformKeyCombo
): combo is PlatformKeyCombo {
	return 'default' in combo;
}
