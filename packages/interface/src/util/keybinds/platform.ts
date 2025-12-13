import type { KeyCombo, Modifier, Platform, PlatformKeyCombo } from './types';
import { isPlatformKeyCombo } from './types';

// Detect current platform
export function getCurrentPlatform(): Platform {
	if (typeof window === 'undefined') return 'web';

	const ua = window.navigator.userAgent;
	if (ua.includes('Mac')) return 'macos';
	if (ua.includes('Win')) return 'windows';
	if (ua.includes('Linux')) return 'linux';
	return 'web';
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

// Convert to Tauri accelerator string
export function toTauriAccelerator(combo: KeyCombo, platform: Platform): string {
	const normalizedModifiers = normalizeModifiers(combo.modifiers, platform);

	const modifierStr = normalizedModifiers
		.map((m) => {
			switch (m) {
				case 'Cmd':
					return platform === 'macos' ? 'Cmd' : 'Ctrl';
				case 'Ctrl':
					return 'Ctrl';
				case 'Alt':
					return 'Alt';
				case 'Shift':
					return 'Shift';
			}
		})
		.join('+');

	// Map key names to Tauri accelerator format
	let key: string;
	switch (combo.key) {
		case 'Space':
			key = 'Space';
			break;
		case 'ArrowUp':
			key = 'Up';
			break;
		case 'ArrowDown':
			key = 'Down';
			break;
		case 'ArrowLeft':
			key = 'Left';
			break;
		case 'ArrowRight':
			key = 'Right';
			break;
		case 'Backspace':
			key = 'Backspace';
			break;
		case 'Delete':
			key = 'Delete';
			break;
		case 'Enter':
			key = 'Enter';
			break;
		case 'Escape':
			key = 'Escape';
			break;
		case 'Tab':
			key = 'Tab';
			break;
		default:
			key = combo.key.toUpperCase();
	}

	return modifierStr ? `${modifierStr}+${key}` : key;
}

// Convert to display string (for context menus)
export function toDisplayString(combo: KeyCombo, platform: Platform): string {
	const normalizedModifiers = normalizeModifiers(combo.modifiers, platform);

	const symbols = normalizedModifiers.map((m) => {
		if (platform === 'macos') {
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
		} else {
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
		}
	});

	// Map key names to display format
	let key: string;
	switch (combo.key) {
		case 'ArrowUp':
			key = platform === 'macos' ? '\u2191' : 'Up';
			break;
		case 'ArrowDown':
			key = platform === 'macos' ? '\u2193' : 'Down';
			break;
		case 'ArrowLeft':
			key = platform === 'macos' ? '\u2190' : 'Left';
			break;
		case 'ArrowRight':
			key = platform === 'macos' ? '\u2192' : 'Right';
			break;
		case 'Backspace':
			key = platform === 'macos' ? '\u232B' : 'Backspace';
			break;
		case 'Delete':
			key = platform === 'macos' ? '\u2326' : 'Del';
			break;
		case 'Enter':
			key = platform === 'macos' ? '\u23CE' : 'Enter';
			break;
		case 'Escape':
			key = platform === 'macos' ? '\u238B' : 'Esc';
			break;
		case 'Tab':
			key = platform === 'macos' ? '\u21E5' : 'Tab';
			break;
		case 'Space':
			key = platform === 'macos' ? '\u2423' : 'Space';
			break;
		default:
			key = combo.key.toUpperCase();
	}

	return platform === 'macos' ? symbols.join('') + key : symbols.join('+') + (symbols.length ? '+' : '') + key;
}

// Match a keyboard event against a key combo
export function matchesKeyCombo(event: KeyboardEvent, combo: KeyCombo, platform: Platform): boolean {
	const normalizedModifiers = normalizeModifiers(combo.modifiers, platform);

	// Check each required modifier
	for (const modifier of normalizedModifiers) {
		switch (modifier) {
			case 'Cmd':
				// On macOS, Cmd = metaKey; on Windows/Linux, Cmd normalizes to Ctrl
				if (platform === 'macos') {
					if (!event.metaKey) return false;
				} else {
					if (!event.ctrlKey) return false;
				}
				break;
			case 'Ctrl':
				if (!event.ctrlKey) return false;
				break;
			case 'Alt':
				if (!event.altKey) return false;
				break;
			case 'Shift':
				if (!event.shiftKey) return false;
				break;
		}
	}

	// Check that no extra modifiers are pressed (unless they're in the combo)
	const hasCmd = normalizedModifiers.includes('Cmd');
	const hasCtrl = normalizedModifiers.includes('Ctrl');
	const hasAlt = normalizedModifiers.includes('Alt');
	const hasShift = normalizedModifiers.includes('Shift');

	// On macOS, check metaKey for Cmd
	if (platform === 'macos') {
		if (!hasCmd && event.metaKey) return false;
		if (!hasCtrl && event.ctrlKey) return false;
	} else {
		// On other platforms, Cmd normalizes to Ctrl
		if (!hasCmd && !hasCtrl && (event.metaKey || event.ctrlKey)) return false;
	}
	if (!hasAlt && event.altKey) return false;
	if (!hasShift && event.shiftKey) return false;

	// Map event.key to our Key type for comparison
	const eventKey = normalizeEventKey(event.key);

	return eventKey.toLowerCase() === combo.key.toLowerCase();
}

// Normalize event.key to our Key format
function normalizeEventKey(key: string): string {
	// Handle special keys
	switch (key) {
		case ' ':
			return 'Space';
		case 'Up':
			return 'ArrowUp';
		case 'Down':
			return 'ArrowDown';
		case 'Left':
			return 'ArrowLeft';
		case 'Right':
			return 'ArrowRight';
		case 'Esc':
			return 'Escape';
		case 'Del':
			return 'Delete';
		case 'Return':
			return 'Enter';
		default:
			return key;
	}
}
