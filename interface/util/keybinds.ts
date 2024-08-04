import { capitalize } from '@sd/client';
import { keySymbols, ModifierKeys, modifierSymbols } from '@sd/ui';

import { OperatingSystem } from '../util/Platform';

export function keybind<T extends string>(
	modifiers: ModifierKeys[],
	keys: T[],
	tauriOs: OperatingSystem
) {
	if (keys.length === 0) return '';

	const os = tauriOs === 'macOS' ? 'macOS' : tauriOs === 'windows' ? 'Windows' : 'Other';

	const keySymbol = keys.map(capitalize).map((key) => {
		const symbol = keySymbols[key];
		return symbol ? symbol[os] ?? symbol.Other : key;
	});

	if (os === 'macOS' && !modifiers.includes(ModifierKeys.Meta)) {
		const index = modifiers.findIndex((modifier) => modifier === ModifierKeys.Control);
		if (index !== -1) modifiers[index] = ModifierKeys.Meta;
	}

	const modifierSymbol = modifiers.map((modifier) => {
		const symbol = modifierSymbols[modifier];
		return symbol[os] ?? symbol.Other;
	});

	const value = [...modifierSymbol, ...keySymbol].join(os === 'macOS' ? '' : '+');

	//we don't want modifier symbols and key symbols to be duplicated if they are the same value
	const noDuplicates = [...new Set(value.split('+'))].join('+');

	return noDuplicates;
}

// Required to export keybind without importing @sd/ui
export type { ModifierKeys } from '@sd/ui';

export function keybindForOs(
	os: OperatingSystem
): (modifiers: ModifierKeys[], keys: string[]) => string {
	return (modifiers: ModifierKeys[], keys: string[]) => keybind(modifiers, keys, os);
}
