import type { KeyCombo, Modifier, Platform, PlatformKeyCombo } from './types';
import { isPlatformKeyCombo } from './types';

// Cache the platform detection result
let cachedPlatform: Platform | null = null;

// Detect current platform
export function getCurrentPlatform(): Platform {
	if (cachedPlatform !== null) {
		return cachedPlatform;
	}

	if (typeof window === 'undefined') {
		cachedPlatform = 'web';
		return cachedPlatform;
	}

	const ua = window.navigator.userAgent;
	if (ua.includes('Mac')) {
		cachedPlatform = 'macos';
	} else if (ua.includes('Win')) {
		cachedPlatform = 'windows';
	} else if (ua.includes('Linux')) {
		cachedPlatform = 'linux';
	} else {
		cachedPlatform = 'web';
	}

	return cachedPlatform;
}

// Get platform-specific combo
export function getComboForPlatform(
	combo: KeyCombo | PlatformKeyCombo,
	platform: Platform = getCurrentPlatform()
): KeyCombo {
	if (isPlatformKeyCombo(combo)) {
		return combo[platform] ?? combo.default;
	}
	return combo;
}

// Normalize Cmd/Ctrl based on platform
export function normalizeModifiers(modifiers: Modifier[], platform: Platform): Modifier[] {
	// On macOS, Cmd stays as Cmd
	// On other platforms, Cmd becomes Ctrl
	if (platform === 'macos') {
		return modifiers;
	}

	return modifiers.map(m => m === 'Cmd' ? 'Ctrl' : m);
}

// Convert to Tauri accelerator string
export function toTauriAccelerator(combo: KeyCombo, platform: Platform): string {
	const normalizedModifiers = normalizeModifiers(combo.modifiers, platform);

	const modifierStr = normalizedModifiers
		.map(m => {
			switch (m) {
				case 'Cmd': return 'Command';
				case 'Ctrl': return 'Control';
				case 'Alt': return 'Alt';
				case 'Shift': return 'Shift';
			}
		})
		.join('+');

	// Convert key to Tauri format
	let key = combo.key;
	// Handle special cases
	if (key === 'ArrowUp') key = 'Up' as typeof key;
	else if (key === 'ArrowDown') key = 'Down' as typeof key;
	else if (key === 'ArrowLeft') key = 'Left' as typeof key;
	else if (key === 'ArrowRight') key = 'Right' as typeof key;
	else if (key === 'Space') key = 'Space' as typeof key;

	return modifierStr ? `${modifierStr}+${key}` : key;
}

// Convert to display string (for context menus)
export function toDisplayString(combo: KeyCombo, platform: Platform): string {
	const normalizedModifiers = normalizeModifiers(combo.modifiers, platform);

	const symbols = normalizedModifiers.map(m => {
		if (platform === 'macos') {
			switch (m) {
				case 'Cmd': return '\u2318'; // ⌘
				case 'Ctrl': return '\u2303'; // ⌃
				case 'Alt': return '\u2325'; // ⌥
				case 'Shift': return '\u21E7'; // ⇧
			}
		} else {
			switch (m) {
				case 'Cmd': return 'Ctrl';
				case 'Ctrl': return 'Ctrl';
				case 'Alt': return 'Alt';
				case 'Shift': return 'Shift';
			}
		}
	});

	// Format key for display
	let displayKey = combo.key;
	if (displayKey === 'ArrowUp') displayKey = '\u2191' as typeof displayKey; // ↑
	else if (displayKey === 'ArrowDown') displayKey = '\u2193' as typeof displayKey; // ↓
	else if (displayKey === 'ArrowLeft') displayKey = '\u2190' as typeof displayKey; // ←
	else if (displayKey === 'ArrowRight') displayKey = '\u2192' as typeof displayKey; // →
	else if (displayKey === 'Backspace') displayKey = '\u232B' as typeof displayKey; // ⌫
	else if (displayKey === 'Delete') displayKey = '\u2326' as typeof displayKey; // ⌦
	else if (displayKey === 'Enter') displayKey = '\u23CE' as typeof displayKey; // ⏎
	else if (displayKey === 'Escape') displayKey = 'Esc' as typeof displayKey;
	else if (displayKey === 'Tab') displayKey = '\u21E5' as typeof displayKey; // ⇥
	else if (displayKey === 'Space') displayKey = 'Space' as typeof displayKey;
	else {
		// Capitalize single letters
		displayKey = displayKey.toUpperCase() as typeof displayKey;
	}

	if (platform === 'macos') {
		// macOS: symbols concatenated without separators
		return symbols.join('') + displayKey;
	} else {
		// Windows/Linux: use + as separator
		if (symbols.length > 0) {
			return symbols.join('+') + '+' + displayKey;
		}
		return displayKey;
	}
}

// Check if an input element is focused (to avoid triggering keybinds when typing)
export function isInputFocused(): boolean {
	if (typeof document === 'undefined') return false;

	const activeElement = document.activeElement;
	if (!activeElement) return false;

	const tagName = activeElement.tagName.toLowerCase();
	if (tagName === 'input' || tagName === 'textarea' || tagName === 'select') {
		return true;
	}

	// Check for contenteditable
	if (activeElement.getAttribute('contenteditable') === 'true') {
		return true;
	}

	return false;
}
