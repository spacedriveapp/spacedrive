import { capitalize } from '@sd/client';
import { keySymbols, ModifierKeys, modifierSymbols } from '@sd/ui';

import { OperatingSystem } from '../util/Platform';

export function keybind<T extends string>(
	modifers: ModifierKeys[],
	keys: T[],
	tauriOs: OperatingSystem
) {
	if (keys.length === 0) return '';

	const os = tauriOs === 'macOS' ? 'macOS' : tauriOs === 'windows' ? 'Windows' : 'Other';

	const keySymbol = keys.map(capitalize).map((key) => {
		const symbol = keySymbols[key];
		return symbol ? (symbol[os] ?? symbol.Other) : key;
	});

	if (os === 'macOS' && !modifers.includes(ModifierKeys.Meta)) {
		const index = modifers.findIndex((modifier) => modifier === ModifierKeys.Control);
		if (index !== -1) modifers[index] = ModifierKeys.Meta;
	}

	const modifierSymbol = modifers.map((modifier) => {
		const symbol = modifierSymbols[modifier];
		return symbol[os] ?? symbol.Other;
	});

	const value = [...modifierSymbol, ...keySymbol].join(os === 'macOS' ? '' : '+');

	//we don't want modifer symbols and key symbols to be duplicated if they are the same value
	const noDuplicates = [...new Set(value.split('+'))].join('+');

	return noDuplicates;
}

// Required to export keybind without importing @sd/ui
export type { ModifierKeys } from '@sd/ui';

export function keybindForOs(
	os: OperatingSystem
): (modifers: ModifierKeys[], keys: string[]) => string {
	return (modifers: ModifierKeys[], keys: string[]) => keybind(modifers, keys, os);
}
