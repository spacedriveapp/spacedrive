import { ModifierKeys, modifierSymbols } from '@sd/ui';

import { OperatingSystem } from '..';
import { useOperatingSystem } from './useOperatingSystem';

type keysToMatch = 'Meta' | 'Alt' | 'Shift';
type keysOsMap = Record<keysToMatch, osKeys>;
type osKeys = Record<OperatingSystem, { key: Partial<keyof typeof ModifierKeys>; icon: string }>;

//This is a helper function to handle the possibility of a modifier key being undefined due to OS initial check
const modifierKey = (key: keyof typeof ModifierKeys, os: 'Windows' | 'macOS' | 'Other') => {
	return modifierSymbols[key][os] ?? modifierSymbols[key]['Other'];
};

//Match macOS keys to Windows keys and others
const keysOsMap = {
	Meta: {
		macOS: { key: 'Meta', icon: modifierKey(ModifierKeys.Meta, 'macOS') },
		windows: { key: 'Control', icon: modifierKey(ModifierKeys.Control, 'Windows') },
		browser: { key: 'Control', icon: modifierKey(ModifierKeys.Control, 'Windows') },
		linux: { key: 'Control', icon: modifierKey(ModifierKeys.Control, 'Windows') },
		unknown: { key: 'Control', icon: modifierKey(ModifierKeys.Control, 'Windows') }
	},
	Shift: {
		macOS: { key: 'Shift', icon: modifierKey(ModifierKeys.Shift, 'macOS') },
		windows: { key: 'Shift', icon: modifierKey(ModifierKeys.Shift, 'Other') },
		browser: { key: 'Shift', icon: modifierKey(ModifierKeys.Shift, 'Other') },
		linux: { key: 'Shift', icon: modifierKey(ModifierKeys.Shift, 'Other') },
		unknown: { key: 'Shift', icon: modifierKey(ModifierKeys.Shift, 'Other') }
	},
	Alt: {
		macOS: { key: 'Alt', icon: modifierKey(ModifierKeys.Alt, 'macOS') },
		windows: { key: 'Alt', icon: modifierKey(ModifierKeys.Alt, 'Other') },
		browser: { key: 'Alt', icon: modifierKey(ModifierKeys.Alt, 'Other') },
		linux: { key: 'Alt', icon: modifierKey(ModifierKeys.Alt, 'Other') },
		unknown: { key: 'Alt', icon: modifierKey(ModifierKeys.Alt, 'Other') }
	}
};

export function useKeyMatcher<T extends keysToMatch>(arg: T): { key: string; icon: string } {
	const os = useOperatingSystem();
	const key = keysOsMap[arg][os];
	return key;
}

//This is another hook to pass an array for multiple keys rather than one at a time
export function useKeysMatcher<T extends keysToMatch>(
	arg: T[]
): Record<T, { key: string; icon: string }> {
	const os = useOperatingSystem();
	const object = {} as Record<T, { key: string; icon: string }>;
	for (const key of arg) {
		object[key] = {
			key: keysOsMap[key][os].key,
			icon: keysOsMap[key][os].icon
		};
	}
	return object;
}
