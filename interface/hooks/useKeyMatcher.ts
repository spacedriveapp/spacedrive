import { EditingKeys, ModifierKeys, modifierSymbols, NavigationKeys, UIKeys } from '@sd/ui';

import { OperatingSystem } from '..';
import { useOperatingSystem } from './useOperatingSystem';

type keyTypes =
	| keyof typeof ModifierKeys
	| keyof typeof EditingKeys
	| keyof typeof UIKeys
	| keyof typeof NavigationKeys;

//This is a helper function to handle the possibility of a modifier key being undefined due to OS initial check
const modifierKey = (key: keyTypes, os: 'Windows' | 'macOS' | 'Other') => {
	return modifierSymbols[key][os] ?? modifierSymbols[key]['Other'];
};

//Match macOS keys to Windows keys and others
const keysOsMap: {
	[T in keyTypes]?: {
		[T in os]?: { key: string; icon: string };
	};
} = {
	Meta: {
		macOS: { key: 'Meta', icon: modifierKey(ModifierKeys.Meta, 'macOS') },
		all: { key: 'Control', icon: modifierKey(ModifierKeys.Control, 'Windows') }
	},
	Shift: {
		macOS: { key: 'Shift', icon: modifierKey(ModifierKeys.Shift, 'macOS') },
		all: { key: 'Shift', icon: modifierKey(ModifierKeys.Shift, 'Other') }
	},
	Alt: {
		macOS: { key: 'Alt', icon: modifierKey(ModifierKeys.Alt, 'macOS') },
		all: { key: 'Alt', icon: modifierKey(ModifierKeys.Alt, 'Other') }
	},
	Escape: {
		macOS: { key: 'Escape', icon: modifierKey(UIKeys.Escape, 'macOS') },
		all: { key: 'Escape', icon: modifierKey(UIKeys.Escape, 'Other') }
	},
	Delete: {
		macOS: { key: 'Delete', icon: modifierKey(EditingKeys.Delete, 'macOS') },
		all: { key: 'Delete', icon: modifierKey(EditingKeys.Delete, 'Other') }
	},
	Backspace: {
		macOS: { key: 'Backspace', icon: modifierKey(EditingKeys.Backspace, 'macOS') },
		all: { key: 'Backspace', icon: modifierKey(EditingKeys.Backspace, 'Other') }
	},
	ArrowUp: {
		all: { key: 'ArrowUp', icon: modifierKey(NavigationKeys.ArrowUp, 'Other') }
	},
	ArrowDown: {
		all: { key: 'ArrowDown', icon: modifierKey(NavigationKeys.ArrowDown, 'Other') }
	},
	ArrowLeft: {
		all: { key: 'ArrowLeft', icon: modifierKey(NavigationKeys.ArrowLeft, 'Other') }
	},
	ArrowRight: {
		all: { key: 'ArrowRight', icon: modifierKey(NavigationKeys.ArrowRight, 'Other') }
	}
};

type keysOfOsMap = keyof typeof keysOsMap;
type os = Exclude<OperatingSystem, 'linux' | 'browser' | 'unknown'> | 'all';

export function useKeyMatcher<T extends keysOfOsMap>(arg: T): { key: string; icon: string } {
	const os = useOperatingSystem(true) as os;
	const key = keysOsMap[arg]?.[os] ?? keysOsMap[arg]?.['all'];
	if (!key) {
		throw new Error(`No key found for ${arg} on ${os}`);
	}
	return key;
}

//This is another hook to pass an array for multiple keys rather than one at a time
export function useKeysMatcher<T extends keysOfOsMap>(
	arg: T[]
): Record<T, { key: string; icon: string }> {
	const os = useOperatingSystem(true) as os;
	const object = {} as Record<T, { key: string; icon: string }>;
	for (const key of arg) {
		object[key] = {
			key: (keysOsMap[key]?.[os]?.key as string) ?? keysOsMap[key]?.['all']?.key,
			icon: (keysOsMap[key]?.[os]?.icon as string) ?? keysOsMap[key]?.['all']?.icon
		};
	}
	return object;
}
