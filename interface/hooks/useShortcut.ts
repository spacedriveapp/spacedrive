import { useMemo } from 'react';
import { useKeys } from 'rooks';
import { useSnapshot } from 'valtio';
import { valtioPersist } from '@sd/client';
import { useRoutingContext } from '~/RoutingContext';
import { OperatingSystem } from '~/util/Platform';

import { useOperatingSystem } from './useOperatingSystem';

type Shortcut = Partial<Record<OperatingSystem | 'all', string[]>>;

const shortcuts = {
	newTab: {
		macOS: ['Meta', 'KeyT'],
		all: ['Control', 'KeyT']
	},
	closeTab: {
		macOS: ['Meta', 'KeyW'],
		all: ['Control', 'KeyW']
	},
	nextTab: {
		macOS: ['Meta', 'Alt', 'ArrowRight'],
		all: ['Control', 'Alt', 'ArrowRight']
	},
	toggleCommandPalette: {
		macOS: ['Meta', 'KeyK'],
		all: ['Control', 'KeyK']
	},
	closeCommandPalette: {
		all: ['Escape']
	},
	previousTab: {
		macOS: ['Meta', 'Alt', 'ArrowLeft'],
		all: ['Control', 'Alt', 'ArrowLeft']
	},
	toggleJobManager: {
		macOS: ['Meta', 'KeyJ'],
		all: ['Control', 'KeyJ']
	},
	toggleTagAssignMode: {
		macOS: ['Meta', 'Alt', 'KeyT'],
		all: ['Control', 'Alt', 'KeyT']
	},
	navBackwardHistory: {
		macOS: ['Meta', '['],
		all: ['Control', '[']
	},
	navForwardHistory: {
		macOS: ['Meta', ']'],
		all: ['Control', ']']
	},
	navToSettings: {
		macOS: ['Shift', 'Meta', 'KeyT'],
		all: ['Shift', 'Control', 'KeyT']
	},
	gridView: {
		macOS: ['Meta', '1'],
		all: ['Control', '1']
	},
	listView: {
		macOS: ['Meta', '2'],
		all: ['Control', '2']
	},
	mediaView: {
		macOS: ['Meta', '3'],
		all: ['Control', '3']
	},
	showHiddenFiles: {
		macOS: ['Meta', 'Shift', '.'],
		all: ['Control', 'KeyH']
	},
	showPathBar: {
		macOS: ['Alt', 'Meta', 'KeyP'],
		all: ['Alt', 'Control', 'KeyP']
	},
	showImageSlider: {
		macOS: ['Alt', 'Meta', 'KeyM'],
		all: ['Alt', 'Control', 'KeyM']
	},
	showInspector: {
		macOS: ['Meta', 'KeyI'],
		all: ['Control', 'KeyI']
	},
	toggleQuickPreview: {
		all: [' ']
	},
	toggleMetaData: {
		macOS: ['Meta', 'KeyI'],
		all: ['Control', 'KeyI']
	},
	quickPreviewMoveBack: {
		all: ['ArrowLeft']
	},
	quickPreviewMoveForward: {
		all: ['ArrowRight']
	},
	revealNative: {
		macOS: ['Meta', 'KeyY'],
		all: ['Control', 'KeyY']
	},
	renameObject: {
		macOS: ['Enter'],
		all: ['F2']
	},
	rescan: {
		macOS: ['Meta', 'KeyR'],
		all: ['Control', 'KeyR']
	},
	cutObject: {
		macOS: ['Meta', 'KeyX'],
		all: ['Control', 'KeyX']
	},
	copyObject: {
		macOS: ['Meta', 'KeyC'],
		all: ['Control', 'KeyC']
	},
	pasteObject: {
		macOS: ['Meta', 'KeyV'],
		all: ['Control', 'KeyV']
	},
	duplicateObject: {
		macOS: ['Meta', 'KeyD'],
		all: ['Control', 'KeyD']
	},
	openObject: {
		macOS: ['Meta', 'KeyO'],
		all: ['Enter']
	},
	quickPreviewOpenNative: {
		macOS: ['Meta', 'KeyO'],
		all: ['Enter']
	},
	closeQuickPreview: {
		all: ['Escape']
	},
	delItem: {
		macOS: ['Meta', 'Backspace'],
		all: ['Delete']
	},
	explorerEscape: {
		all: ['Escape']
	},
	explorerDown: {
		all: ['ArrowDown']
	},
	explorerUp: {
		all: ['ArrowUp']
	},
	explorerLeft: {
		all: ['ArrowLeft']
	},
	explorerRight: {
		all: ['ArrowRight']
	},
	toggleSidebar: {
		all: ['[']
	}
} satisfies Record<string, Shortcut>;

export type Shortcuts = keyof typeof shortcuts;

export const shortcutsStore = valtioPersist(
	'sd-shortcuts',
	shortcuts as Record<Shortcuts, Shortcut>
);

export const useShortcut = (
	shortcut: Shortcuts,
	func: (e: KeyboardEvent) => void,
	options: Omit<Parameters<typeof useKeys>[2], 'when'> & { disabled?: boolean } = {}
) => {
	const os = useOperatingSystem(true);
	const shortcuts = useSnapshot(shortcutsStore);
	const { visible } = useRoutingContext();

	const keys = useMemo(() => {
		if (!visible) return [];
		return shortcuts[shortcut][os] ?? shortcuts[shortcut].all ?? [];
	}, [os, shortcut, shortcuts, visible]);

	// useKeys doesn't like readonly
	useKeys(
		keys as string[],
		(e) => {
			if (!import.meta.env.DEV) e.preventDefault();
			return func(e);
		},
		{
			...options,
			when: visible && !options.disabled
		}
	);
};
