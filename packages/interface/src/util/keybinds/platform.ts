/**
 * Unified Keybind System - Platform Utilities
 *
 * Platform detection and key combo conversion utilities.
 * Handles cross-platform differences (Cmd on Mac vs Ctrl on Windows/Linux).
 */

import type { KeyCombo, Modifier, Platform, PlatformKeyCombo } from './types';
import { isPlatformKeyCombo } from './types';

/**
 * Detect current platform from user agent
 */
export function getCurrentPlatform(): Platform {
	if (typeof window === 'undefined') return 'web';

	const ua = window.navigator.userAgent;
	if (ua.includes('Mac')) return 'macos';
	if (ua.includes('Win')) return 'windows';
	if (ua.includes('Linux')) return 'linux';
	return 'web';
}

/**
 * Get the platform-specific combo from a possibly platform-specific definition
 */
export function getComboForPlatform(
	combo: KeyCombo | PlatformKeyCombo,
	platform: Platform = getCurrentPlatform()
): KeyCombo {
	if (isPlatformKeyCombo(combo)) {
		return combo[platform] ?? combo.default;
	}
	return combo;
}

/**
 * Normalize modifiers based on platform
 * - On macOS: Cmd stays as Cmd (meta key)
 * - On Windows/Linux: Cmd becomes Ctrl
 */
export function normalizeModifiers(modifiers: Modifier[], platform: Platform): Modifier[] {
	// Replace Cmd with Ctrl on non-macOS
	if (platform !== 'macos' && modifiers.includes('Cmd')) {
		return modifiers.map((m) => (m === 'Cmd' ? 'Ctrl' : m));
	}
	return [...modifiers];
}

/**
 * Convert key combo to Tauri accelerator string format
 * Used when registering with Tauri global shortcuts
 */
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

	// Map special keys to Tauri accelerator format
	let key: string = combo.key;
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
		default:
			key = combo.key;
	}

	return modifierStr ? `${modifierStr}+${key}` : key;
}

/**
 * Convert key combo to display string for UI
 * Uses platform-appropriate symbols (macOS uses symbols, others use text)
 */
export function toDisplayString(combo: KeyCombo, platform: Platform): string {
	const normalizedModifiers = normalizeModifiers(combo.modifiers, platform);

	const symbols = normalizedModifiers.map((m) => {
		if (platform === 'macos') {
			switch (m) {
				case 'Cmd':
					return '\u2318'; // Command symbol
				case 'Ctrl':
					return '\u2303'; // Control symbol
				case 'Alt':
					return '\u2325'; // Option symbol
				case 'Shift':
					return '\u21E7'; // Shift symbol
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

	// Format key for display
	let displayKey: string;
	switch (combo.key) {
		case 'ArrowUp':
			displayKey = platform === 'macos' ? '\u2191' : 'Up';
			break;
		case 'ArrowDown':
			displayKey = platform === 'macos' ? '\u2193' : 'Down';
			break;
		case 'ArrowLeft':
			displayKey = platform === 'macos' ? '\u2190' : 'Left';
			break;
		case 'ArrowRight':
			displayKey = platform === 'macos' ? '\u2192' : 'Right';
			break;
		case 'Backspace':
			displayKey = platform === 'macos' ? '\u232B' : 'Backspace';
			break;
		case 'Delete':
			displayKey = platform === 'macos' ? '\u2326' : 'Del';
			break;
		case 'Enter':
			displayKey = platform === 'macos' ? '\u23CE' : 'Enter';
			break;
		case 'Escape':
			displayKey = platform === 'macos' ? '\u238B' : 'Esc';
			break;
		case 'Tab':
			displayKey = platform === 'macos' ? '\u21E5' : 'Tab';
			break;
		case 'Space':
			displayKey = platform === 'macos' ? '\u2423' : 'Space';
			break;
		default:
			// Capitalize single letter keys
			displayKey = combo.key.length === 1 ? combo.key.toUpperCase() : combo.key;
	}

	if (platform === 'macos') {
		// macOS: symbols joined without separator + key
		return symbols.join('') + displayKey;
	} else {
		// Windows/Linux: modifiers joined with + and then + key
		if (symbols.length > 0) {
			return symbols.join('+') + '+' + displayKey;
		}
		return displayKey;
	}
}

/**
 * Check if a KeyboardEvent matches a KeyCombo
 */
export function eventMatchesCombo(e: KeyboardEvent, combo: KeyCombo, platform: Platform): boolean {
	const normalizedModifiers = normalizeModifiers(combo.modifiers, platform);

	// Check each required modifier
	for (const modifier of normalizedModifiers) {
		switch (modifier) {
			case 'Cmd':
				// On macOS, Cmd = metaKey; on others, Cmd normalizes to Ctrl
				if (platform === 'macos') {
					if (!e.metaKey) return false;
				} else {
					if (!e.ctrlKey) return false;
				}
				break;
			case 'Ctrl':
				if (!e.ctrlKey) return false;
				break;
			case 'Alt':
				if (!e.altKey) return false;
				break;
			case 'Shift':
				if (!e.shiftKey) return false;
				break;
		}
	}

	// Check that no extra modifiers are pressed
	const hasCmd = normalizedModifiers.includes('Cmd');
	const hasCtrl = normalizedModifiers.includes('Ctrl');
	const hasAlt = normalizedModifiers.includes('Alt');
	const hasShift = normalizedModifiers.includes('Shift');

	// Check for unwanted modifier keys
	if (!hasCmd && !hasCtrl) {
		// Neither Cmd nor Ctrl expected
		if (platform === 'macos' && e.metaKey) return false;
		if (e.ctrlKey) return false;
	} else if (hasCmd && !hasCtrl) {
		// Only Cmd expected (or Ctrl on non-Mac)
		if (platform === 'macos') {
			// On macOS, Cmd uses metaKey, ctrl should not be pressed
			if (e.ctrlKey) return false;
		}
		// On non-Mac, Cmd normalizes to Ctrl, so metaKey should not be pressed
		if (platform !== 'macos' && e.metaKey) return false;
	} else if (!hasCmd && hasCtrl) {
		// Only Ctrl expected
		if (e.metaKey) return false;
	}

	if (!hasAlt && e.altKey) return false;
	if (!hasShift && e.shiftKey) return false;

	// Check key - normalize both to lowercase for comparison
	const eventKey = e.key.toLowerCase();
	const comboKey = combo.key.toLowerCase();

	// Handle special key mappings
	if (combo.key === 'Space' && e.key === ' ') return true;

	return eventKey === comboKey;
}
