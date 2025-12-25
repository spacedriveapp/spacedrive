import { defineKeybind } from './types';
import type { KeybindDefinition, KeybindScope } from './types';

// Explorer keybinds
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
		scope: 'explorer',
		preventDefault: true
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
	enterTagMode: defineKeybind({
		id: 'explorer.enterTagMode',
		label: 'Enter Tag Mode',
		combo: { modifiers: [], key: 't' },
		scope: 'explorer'
	}),

	exitTagMode: defineKeybind({
		id: 'explorer.exitTagMode',
		label: 'Exit Tag Mode',
		combo: { modifiers: [], key: 'Escape' },
		scope: 'explorer'
	}),

	toggleTag1: defineKeybind({
		id: 'explorer.toggleTag1',
		label: 'Toggle Tag 1',
		combo: { modifiers: [], key: '1' },
		scope: 'explorer'
	}),

	toggleTag2: defineKeybind({
		id: 'explorer.toggleTag2',
		label: 'Toggle Tag 2',
		combo: { modifiers: [], key: '2' },
		scope: 'explorer'
	}),

	toggleTag3: defineKeybind({
		id: 'explorer.toggleTag3',
		label: 'Toggle Tag 3',
		combo: { modifiers: [], key: '3' },
		scope: 'explorer'
	}),

	toggleTag4: defineKeybind({
		id: 'explorer.toggleTag4',
		label: 'Toggle Tag 4',
		combo: { modifiers: [], key: '4' },
		scope: 'explorer'
	}),

	toggleTag5: defineKeybind({
		id: 'explorer.toggleTag5',
		label: 'Toggle Tag 5',
		combo: { modifiers: [], key: '5' },
		scope: 'explorer'
	}),

	toggleTag6: defineKeybind({
		id: 'explorer.toggleTag6',
		label: 'Toggle Tag 6',
		combo: { modifiers: [], key: '6' },
		scope: 'explorer'
	}),

	toggleTag7: defineKeybind({
		id: 'explorer.toggleTag7',
		label: 'Toggle Tag 7',
		combo: { modifiers: [], key: '7' },
		scope: 'explorer'
	}),

	toggleTag8: defineKeybind({
		id: 'explorer.toggleTag8',
		label: 'Toggle Tag 8',
		combo: { modifiers: [], key: '8' },
		scope: 'explorer'
	}),

	toggleTag9: defineKeybind({
		id: 'explorer.toggleTag9',
		label: 'Toggle Tag 9',
		combo: { modifiers: [], key: '9' },
		scope: 'explorer'
	}),

	toggleTag10: defineKeybind({
		id: 'explorer.toggleTag10',
		label: 'Toggle Tag 10',
		combo: { modifiers: [], key: '0' },
		scope: 'explorer'
	}),

	// Clear selection
	clearSelection: defineKeybind({
		id: 'explorer.clearSelection',
		label: 'Clear Selection',
		combo: { modifiers: [], key: 'Escape' },
		scope: 'explorer'
	}),
} as const;

// Global keybinds
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

	newTab: defineKeybind({
		id: 'global.newTab',
		label: 'New Tab',
		combo: { modifiers: ['Cmd'], key: 't' },
		scope: 'global',
		preventDefault: true
	}),

	closeTab: defineKeybind({
		id: 'global.closeTab',
		label: 'Close Tab',
		combo: { modifiers: ['Cmd'], key: 'w' },
		scope: 'global',
		preventDefault: true
	}),

	nextTab: defineKeybind({
		id: 'global.nextTab',
		label: 'Next Tab',
		combo: { modifiers: ['Cmd', 'Shift'], key: ']' },
		scope: 'global'
	}),

	previousTab: defineKeybind({
		id: 'global.previousTab',
		label: 'Previous Tab',
		combo: { modifiers: ['Cmd', 'Shift'], key: '[' },
		scope: 'global'
	}),

	selectTab1: defineKeybind({
		id: 'global.selectTab1',
		label: 'Go to Tab 1',
		combo: { modifiers: ['Cmd'], key: '1' },
		scope: 'global'
	}),

	selectTab2: defineKeybind({
		id: 'global.selectTab2',
		label: 'Go to Tab 2',
		combo: { modifiers: ['Cmd'], key: '2' },
		scope: 'global'
	}),

	selectTab3: defineKeybind({
		id: 'global.selectTab3',
		label: 'Go to Tab 3',
		combo: { modifiers: ['Cmd'], key: '3' },
		scope: 'global'
	}),

	selectTab4: defineKeybind({
		id: 'global.selectTab4',
		label: 'Go to Tab 4',
		combo: { modifiers: ['Cmd'], key: '4' },
		scope: 'global'
	}),

	selectTab5: defineKeybind({
		id: 'global.selectTab5',
		label: 'Go to Tab 5',
		combo: { modifiers: ['Cmd'], key: '5' },
		scope: 'global'
	}),

	selectTab6: defineKeybind({
		id: 'global.selectTab6',
		label: 'Go to Tab 6',
		combo: { modifiers: ['Cmd'], key: '6' },
		scope: 'global'
	}),

	selectTab7: defineKeybind({
		id: 'global.selectTab7',
		label: 'Go to Tab 7',
		combo: { modifiers: ['Cmd'], key: '7' },
		scope: 'global'
	}),

	selectTab8: defineKeybind({
		id: 'global.selectTab8',
		label: 'Go to Tab 8',
		combo: { modifiers: ['Cmd'], key: '8' },
		scope: 'global'
	}),

	selectTab9: defineKeybind({
		id: 'global.selectTab9',
		label: 'Go to Tab 9',
		combo: { modifiers: ['Cmd'], key: '9' },
		scope: 'global'
	}),

	focusSearchBar: defineKeybind({
		id: 'global.focusSearchBar',
		label: 'Focus Search Bar',
		combo: { modifiers: ['Cmd'], key: 'f' },
		scope: 'global',
		preventDefault: true
	}),
} as const;

// Media viewer keybinds
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
	}),
} as const;

// Quick preview keybinds
export const quickPreviewKeybinds = {
	close: defineKeybind({
		id: 'quickPreview.close',
		label: 'Close Preview',
		combo: { modifiers: [], key: 'Escape' },
		scope: 'quickPreview'
	}),

	closeWithSpace: defineKeybind({
		id: 'quickPreview.closeWithSpace',
		label: 'Close Preview',
		combo: { modifiers: [], key: 'Space' },
		scope: 'quickPreview'
	}),
} as const;

// Combined registry
export const KEYBINDS = {
	explorer: explorerKeybinds,
	global: globalKeybinds,
	mediaViewer: mediaViewerKeybinds,
	quickPreview: quickPreviewKeybinds,
} as const;

// Extract all keybind IDs as union type
type KeybindCategory = typeof KEYBINDS;
type ExtractIds<T> = T extends Record<string, KeybindDefinition>
	? T[keyof T]['id']
	: never;

export type KeybindId =
	| ExtractIds<KeybindCategory['explorer']>
	| ExtractIds<KeybindCategory['global']>
	| ExtractIds<KeybindCategory['mediaViewer']>
	| ExtractIds<KeybindCategory['quickPreview']>;

// Helper to get keybind by ID
export function getKeybind(id: KeybindId): KeybindDefinition | undefined {
	for (const category of Object.values(KEYBINDS)) {
		for (const keybind of Object.values(category)) {
			if (keybind.id === id) return keybind;
		}
	}
	return undefined;
}

// Get all keybinds as flat array
export function getAllKeybinds(): KeybindDefinition[] {
	return Object.values(KEYBINDS).flatMap(category => Object.values(category));
}

// Get keybinds by scope
export function getKeybindsByScope(scope: KeybindScope): KeybindDefinition[] {
	return getAllKeybinds().filter(kb => kb.scope === scope);
}
