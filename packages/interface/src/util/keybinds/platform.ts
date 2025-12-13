/**
 * Platform Utilities for Keybinds
 *
 * This module provides platform detection and conversion utilities
 * for rendering keybind strings in the correct format for each platform.
 */

import type { KeyCombo, Modifier, Platform, PlatformKeyCombo } from './types';
import { isPlatformKeyCombo } from './types';

/**
 * Detect the current platform from the user agent.
 * Falls back to 'web' if platform cannot be determined.
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
 * Get the platform-specific key combo.
 * If the combo has platform overrides, returns the override for the current platform
 * or falls back to the default.
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
 * Normalize modifiers for a specific platform.
 * On non-macOS platforms, 'Cmd' is converted to 'Ctrl'.
 */
export function normalizeModifiers(modifiers: Modifier[], platform: Platform): Modifier[] {
	if (platform !== 'macos' && modifiers.includes('Cmd')) {
		return modifiers.map((m) => (m === 'Cmd' ? 'Ctrl' : m));
	}
	return [...modifiers];
}

/**
 * Convert a key combo to a Tauri accelerator string.
 * Format: "Modifier+Modifier+Key" (e.g., "Cmd+Shift+P")
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

	// Map special keys to Tauri format
	const key = mapKeyToTauri(combo.key);

	return modifierStr ? `${modifierStr}+${key}` : key;
}

/**
 * Map a key to Tauri accelerator format
 */
function mapKeyToTauri(key: string): string {
	switch (key) {
		case 'Space':
			return 'Space';
		case 'Enter':
			return 'Return';
		case 'ArrowUp':
			return 'Up';
		case 'ArrowDown':
			return 'Down';
		case 'ArrowLeft':
			return 'Left';
		case 'ArrowRight':
			return 'Right';
		default:
			return key;
	}
}

/**
 * Convert a key combo to a display string for UI (context menus, tooltips).
 * Uses platform-native symbols on macOS, text on other platforms.
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

	const key = formatKeyForDisplay(combo.key, platform);

	if (platform === 'macos') {
		return symbols.join('') + key;
	} else {
		return symbols.length > 0 ? symbols.join('+') + '+' + key : key;
	}
}

/**
 * Format a key for display in the UI
 */
function formatKeyForDisplay(key: string, platform: Platform): string {
	switch (key) {
		case 'ArrowUp':
			return platform === 'macos' ? '\u2191' : 'Up';
		case 'ArrowDown':
			return platform === 'macos' ? '\u2193' : 'Down';
		case 'ArrowLeft':
			return platform === 'macos' ? '\u2190' : 'Left';
		case 'ArrowRight':
			return platform === 'macos' ? '\u2192' : 'Right';
		case 'Backspace':
			return platform === 'macos' ? '\u232B' : 'Backspace';
		case 'Delete':
			return platform === 'macos' ? '\u2326' : 'Delete';
		case 'Enter':
			return platform === 'macos' ? '\u21A9' : 'Enter';
		case 'Escape':
			return platform === 'macos' ? '\u238B' : 'Esc';
		case 'Tab':
			return platform === 'macos' ? '\u21E5' : 'Tab';
		case 'Space':
			return platform === 'macos' ? '\u2423' : 'Space';
		default:
			// Capitalize single letters, leave others as-is
			if (key.length === 1) {
				return key.toUpperCase();
			}
			return key;
	}
}

/**
 * Check if a keyboard event matches a key combo
 */
export function matchesKeyCombo(event: KeyboardEvent, combo: KeyCombo, platform: Platform): boolean {
	const normalizedModifiers = normalizeModifiers(combo.modifiers, platform);

	// Check required modifiers are pressed
	for (const modifier of normalizedModifiers) {
		switch (modifier) {
			case 'Cmd':
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

	// Check no extra modifiers are pressed
	const hasCmd = normalizedModifiers.includes('Cmd');
	const hasCtrl = normalizedModifiers.includes('Ctrl');
	const hasAlt = normalizedModifiers.includes('Alt');
	const hasShift = normalizedModifiers.includes('Shift');

	// On macOS, Cmd uses metaKey; on other platforms, Cmd is normalized to Ctrl
	if (platform === 'macos') {
		if (!hasCmd && event.metaKey) return false;
		if (!hasCtrl && event.ctrlKey) return false;
	} else {
		// On non-macOS, both Cmd and Ctrl map to ctrlKey
		if (!hasCmd && !hasCtrl && event.ctrlKey) return false;
		if (event.metaKey) return false; // Windows key shouldn't interfere
	}

	if (!hasAlt && event.altKey) return false;
	if (!hasShift && event.shiftKey) return false;

	// Check the key
	return matchesKey(event, combo.key);
}

/**
 * Check if a keyboard event matches a specific key
 */
function matchesKey(event: KeyboardEvent, key: string): boolean {
	// Handle special keys
	switch (key) {
		case 'Space':
			return event.code === 'Space' || event.key === ' ';
		case 'Enter':
			return event.key === 'Enter';
		case 'Escape':
			return event.key === 'Escape';
		case 'Backspace':
			return event.key === 'Backspace';
		case 'Delete':
			return event.key === 'Delete';
		case 'Tab':
			return event.key === 'Tab';
		case 'ArrowUp':
			return event.key === 'ArrowUp';
		case 'ArrowDown':
			return event.key === 'ArrowDown';
		case 'ArrowLeft':
			return event.key === 'ArrowLeft';
		case 'ArrowRight':
			return event.key === 'ArrowRight';
		default:
			// For letters, compare case-insensitively
			return event.key.toLowerCase() === key.toLowerCase();
	}
}
