/**
 * Unified Keybind System - Registry
 *
 * Central registry of all keybind definitions.
 * Single source of truth for keyboard shortcuts across the app.
 */

import { defineKeybind } from './types';
import type { KeybindDefinition, KeybindScope } from './types';

// ============================================================================
// Explorer Keybinds
// ============================================================================

export const explorerKeybinds = {
	// File operations
	openFile: defineKeybind({
		id: 'explorer.openFile',
		label: 'Open File',
		combo: { modifiers: ['Cmd'], key: 'o' },
		scope: 'explorer'
	}),

	revealInNativeExplorer: defineKeybind({
		id: 'explorer.revealInNativeExplorer',
		label: 'Reveal in Native Explorer',
		combo: { modifiers: ['Cmd', 'Shift'], key: 'r' },
		scope: 'explorer'
	}),

	renameFile: defineKeybind({
		id: 'explorer.renameFile',
		label: 'Rename',
		combo: { modifiers: [], key: 'Enter' },
		scope: 'explorer'
	}),

	// Selection
	selectAll: defineKeybind({
		id: 'explorer.selectAll',
		label: 'Select All',
		combo: { modifiers: ['Cmd'], key: 'a' },
		scope: 'explorer',
		preventDefault: true
	}),

	// Clipboard
	cut: defineKeybind({
		id: 'explorer.cut',
		label: 'Cut',
		combo: { modifiers: ['Cmd'], key: 'x' },
		scope: 'explorer'
	}),

	copy: defineKeybind({
		id: 'explorer.copy',
		label: 'Copy',
		combo: { modifiers: ['Cmd'], key: 'c' },
		scope: 'explorer'
	}),

	paste: defineKeybind({
		id: 'explorer.paste',
		label: 'Paste',
		combo: { modifiers: ['Cmd'], key: 'v' },
		scope: 'explorer'
	}),

	duplicate: defineKeybind({
		id: 'explorer.duplicate',
		label: 'Duplicate',
		combo: { modifiers: ['Cmd'], key: 'd' },
		scope: 'explorer'
	}),

	// Delete
	delete: defineKeybind({
		id: 'explorer.delete',
		label: 'Delete',
		combo: {
			macos: { modifiers: ['Cmd'], key: 'Backspace' },
			default: { modifiers: [], key: 'Delete' }
		},
		scope: 'explorer'
	}),

	permanentDelete: defineKeybind({
		id: 'explorer.permanentDelete',
		label: 'Permanent Delete',
		combo: {
			macos: { modifiers: ['Cmd', 'Alt'], key: 'Backspace' },
			default: { modifiers: ['Shift'], key: 'Delete' }
		},
		scope: 'explorer'
	}),

	// Navigation
	navigateBack: defineKeybind({
		id: 'explorer.navigateBack',
		label: 'Navigate Back',
		combo: { modifiers: ['Cmd'], key: 'ArrowLeft' },
		scope: 'explorer'
	}),

	navigateForward: defineKeybind({
		id: 'explorer.navigateForward',
		label: 'Navigate Forward',
		combo: { modifiers: ['Cmd'], key: 'ArrowRight' },
		scope: 'explorer'
	}),

	navigateToParent: defineKeybind({
		id: 'explorer.navigateToParent',
		label: 'Navigate to Parent',
		combo: { modifiers: ['Cmd'], key: 'ArrowUp' },
		scope: 'explorer'
	}),

	openInNewTab: defineKeybind({
		id: 'explorer.openInNewTab',
		label: 'Open in New Tab',
		combo: { modifiers: ['Cmd'], key: 't' },
		scope: 'explorer'
	}),

	// View
	toggleMetadata: defineKeybind({
		id: 'explorer.toggleMetadata',
		label: 'Toggle Metadata',
		combo: { modifiers: ['Cmd'], key: 'i' },
		scope: 'explorer'
	}),

	toggleQuickPreview: defineKeybind({
		id: 'explorer.toggleQuickPreview',
		label: 'Quick Preview',
		combo: { modifiers: [], key: 'Space' },
		scope: 'explorer'
	}),

	// Grid/List navigation (arrow keys)
	moveUp: defineKeybind({
		id: 'explorer.moveUp',
		label: 'Move Up',
		combo: { modifiers: [], key: 'ArrowUp' },
		scope: 'explorer'
	}),

	moveDown: defineKeybind({
		id: 'explorer.moveDown',
		label: 'Move Down',
		combo: { modifiers: [], key: 'ArrowDown' },
		scope: 'explorer'
	}),

	moveLeft: defineKeybind({
		id: 'explorer.moveLeft',
		label: 'Move Left',
		combo: { modifiers: [], key: 'ArrowLeft' },
		scope: 'explorer'
	}),

	moveRight: defineKeybind({
		id: 'explorer.moveRight',
		label: 'Move Right',
		combo: { modifiers: [], key: 'ArrowRight' },
		scope: 'explorer'
	}),

	// Multi-select
	extendSelectionUp: defineKeybind({
		id: 'explorer.extendSelectionUp',
		label: 'Extend Selection Up',
		combo: { modifiers: ['Shift'], key: 'ArrowUp' },
		scope: 'explorer'
	}),

	extendSelectionDown: defineKeybind({
		id: 'explorer.extendSelectionDown',
		label: 'Extend Selection Down',
		combo: { modifiers: ['Shift'], key: 'ArrowDown' },
		scope: 'explorer'
	}),

	extendSelectionLeft: defineKeybind({
		id: 'explorer.extendSelectionLeft',
		label: 'Extend Selection Left',
		combo: { modifiers: ['Shift'], key: 'ArrowLeft' },
		scope: 'explorer'
	}),

	extendSelectionRight: defineKeybind({
		id: 'explorer.extendSelectionRight',
		label: 'Extend Selection Right',
		combo: { modifiers: ['Shift'], key: 'ArrowRight' },
		scope: 'explorer'
	}),

	// Tag mode
	toggleTagMode: defineKeybind({
		id: 'explorer.toggleTagMode',
		label: 'Toggle Tag Mode',
		combo: { modifiers: [], key: 't' },
		scope: 'explorer'
	}),

	// Clear selection
	clearSelection: defineKeybind({
		id: 'explorer.clearSelection',
		label: 'Clear Selection',
		combo: { modifiers: [], key: 'Escape' },
		scope: 'explorer'
	})
} as const;

// ============================================================================
// Global Keybinds
// ============================================================================

export const globalKeybinds = {
	openCommandPalette: defineKeybind({
		id: 'global.openCommandPalette',
		label: 'Open Command Palette',
		combo: { modifiers: ['Cmd', 'Shift'], key: 'p' },
		scope: 'global',
		preventDefault: true
	}),

	openSettings: defineKeybind({
		id: 'global.openSettings',
		label: 'Open Settings',
		combo: { modifiers: ['Cmd'], key: ',' },
		scope: 'global'
	}),

	closeTab: defineKeybind({
		id: 'global.closeTab',
		label: 'Close Tab',
		combo: { modifiers: ['Cmd'], key: 'w' },
		scope: 'global'
	}),

	newTab: defineKeybind({
		id: 'global.newTab',
		label: 'New Tab',
		combo: { modifiers: ['Cmd'], key: 't' },
		scope: 'global'
	}),

	focusSearchBar: defineKeybind({
		id: 'global.focusSearchBar',
		label: 'Focus Search Bar',
		combo: { modifiers: ['Cmd'], key: 'f' },
		scope: 'global',
		preventDefault: true
	})
} as const;

// ============================================================================
// Media Viewer Keybinds
// ============================================================================

export const mediaViewerKeybinds = {
	closeViewer: defineKeybind({
		id: 'mediaViewer.close',
		label: 'Close Viewer',
		combo: { modifiers: [], key: 'Escape' },
		scope: 'mediaViewer'
	}),

	nextFile: defineKeybind({
		id: 'mediaViewer.nextFile',
		label: 'Next File',
		combo: { modifiers: [], key: 'ArrowRight' },
		scope: 'mediaViewer'
	}),

	previousFile: defineKeybind({
		id: 'mediaViewer.previousFile',
		label: 'Previous File',
		combo: { modifiers: [], key: 'ArrowLeft' },
		scope: 'mediaViewer'
	})
} as const;

// ============================================================================
// Tag Assigner Keybinds
// ============================================================================

export const tagAssignerKeybinds = {
	close: defineKeybind({
		id: 'tagAssigner.close',
		label: 'Close Tag Assigner',
		combo: { modifiers: [], key: 'Escape' },
		scope: 'tagAssigner'
	})
} as const;

// ============================================================================
// Combined Registry
// ============================================================================

export const KEYBINDS = {
	explorer: explorerKeybinds,
	global: globalKeybinds,
	mediaViewer: mediaViewerKeybinds,
	tagAssigner: tagAssignerKeybinds
} as const;

// ============================================================================
// Type Extraction
// ============================================================================

/**
 * Extract all keybind IDs as a union type for type-safe references
 */
type ExtractKeybindIds<T> = T extends Record<string, Record<string, KeybindDefinition>>
	? {
			[K in keyof T]: {
				[J in keyof T[K]]: T[K][J] extends KeybindDefinition ? T[K][J]['id'] : never;
			}[keyof T[K]];
		}[keyof T]
	: never;

/**
 * Union type of all valid keybind IDs
 * Provides autocomplete when using useKeybind()
 */
export type KeybindId = ExtractKeybindIds<typeof KEYBINDS>;

// ============================================================================
// Registry Helpers
// ============================================================================

/**
 * Get a keybind definition by ID
 */
export function getKeybind(id: KeybindId): KeybindDefinition | undefined {
	for (const category of Object.values(KEYBINDS)) {
		for (const keybind of Object.values(category)) {
			if (keybind.id === id) return keybind;
		}
	}
	return undefined;
}

/**
 * Get all keybinds as a flat array
 */
export function getAllKeybinds(): KeybindDefinition[] {
	return Object.values(KEYBINDS).flatMap((category) => Object.values(category));
}

/**
 * Get all keybinds for a specific scope
 */
export function getKeybindsByScope(scope: KeybindScope): KeybindDefinition[] {
	return getAllKeybinds().filter((kb) => kb.scope === scope);
}

/**
 * Check if an ID is a valid keybind ID
 */
export function isValidKeybindId(id: string): id is KeybindId {
	return getKeybind(id as KeybindId) !== undefined;
}
