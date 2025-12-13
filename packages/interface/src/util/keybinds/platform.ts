import type { KeyCombo, Modifier, Platform, PlatformKeyCombo } from './types';

// Detect current platform
export function getCurrentPlatform(): Platform {
	if (typeof window === 'undefined') return 'web';

	const ua = window.navigator.userAgent;
	if (ua.includes('Mac')) return 'macos';
	if (ua.includes('Win')) return 'windows';
	if (ua.includes('Linux')) return 'linux';
	return 'web';
}

// Check if combo is platform-specific
function isPlatformKeyCombo(combo: KeyCombo | PlatformKeyCombo): combo is PlatformKeyCombo {
	return 'default' in combo;
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
	// Replace Cmd with Ctrl on non-macOS
	if (platform !== 'macos' && modifiers.includes('Cmd')) {
		return modifiers.map((m) => (m === 'Cmd' ? 'Ctrl' : m));
	}

	return modifiers;
}

// Key name mappings for Tauri accelerator format
const TAURI_KEY_MAP: Partial<Record<string, string>> = {
	ArrowUp: 'Up',
	ArrowDown: 'Down',
	ArrowLeft: 'Left',
	ArrowRight: 'Right',
	Escape: 'Escape',
	Enter: 'Enter',
	Backspace: 'Backspace',
	Delete: 'Delete',
	Tab: 'Tab',
	Space: 'Space'
};

// Convert to Tauri accelerator string
export function toTauriAccelerator(combo: KeyCombo, platform: Platform): string {
	const normalizedModifiers = normalizeModifiers(combo.modifiers, platform);

	const modifierStr = normalizedModifiers
		.map((m) => {
			switch (m) {
				case 'Cmd':
					return platform === 'macos' ? 'Command' : 'Ctrl';
				case 'Ctrl':
					return 'Ctrl';
				case 'Alt':
					return 'Alt';
				case 'Shift':
					return 'Shift';
			}
		})
		.join('+');

	const key = TAURI_KEY_MAP[combo.key] ?? combo.key.toUpperCase();

	return modifierStr ? `${modifierStr}+${key}` : key;
}

// Key name mappings for display
const DISPLAY_KEY_MAP: Partial<Record<string, string>> = {
	ArrowUp: '\u2191',
	ArrowDown: '\u2193',
	ArrowLeft: '\u2190',
	ArrowRight: '\u2192',
	Escape: 'Esc',
	Enter: '\u21B5',
	Backspace: '\u232B',
	Delete: 'Del',
	Tab: '\u21E5',
	Space: 'Space'
};

// Convert to display string (for context menus)
export function toDisplayString(combo: KeyCombo, platform: Platform): string {
	const normalizedModifiers = normalizeModifiers(combo.modifiers, platform);

	if (platform === 'macos') {
		const symbols = normalizedModifiers.map((m) => {
			switch (m) {
				case 'Cmd':
					return '\u2318';
				case 'Ctrl':
					return '\u2303';
				case 'Alt':
					return '\u2325';
				case 'Shift':
					return '\u21E7';
			}
		});

		const key = DISPLAY_KEY_MAP[combo.key] ?? combo.key.toUpperCase();
		return symbols.join('') + key;
	} else {
		const symbols = normalizedModifiers.map((m) => {
			switch (m) {
				case 'Cmd':
					return 'Ctrl';
				case 'Ctrl':
					return 'Ctrl';
				case 'Alt':
					return 'Alt';
				case 'Shift':
					return 'Shift';
			}
		});

		const key = DISPLAY_KEY_MAP[combo.key] ?? combo.key.toUpperCase();
		return symbols.length > 0 ? symbols.join('+') + '+' + key : key;
	}
}
