import { ModifierKeys, keySymbols, modifierSymbols } from '@sd/ui';
import { OperatingSystem } from '../util/Platform';

function capitalize<T extends string>(string: T): Capitalize<T> {
	return (string.charAt(0).toUpperCase() + string.slice(1)) as Capitalize<T>;
}

export function keybind<T extends string>(
	modifers: ModifierKeys[],
	keys: T[],
	tauriOs: OperatingSystem
) {
	const os = tauriOs === 'macOS' ? 'macOS' : tauriOs === 'windows' ? 'Windows' : 'Other';

	const keySymbol = keys.map(capitalize).map((key) => {
		const symbol = keySymbols[key];
		return symbol ? symbol[os] ?? symbol.Other : key;
	});

	if (keySymbol.length === 0) return '';

	const modifierSymbol = modifers.map((modifier) => {
		const symbol = modifierSymbols[modifier];
		return symbol[os] ?? symbol.Other;
	});

	return [...modifierSymbol, ...keySymbol].join(os === 'macOS' ? '' : '+');
}

export function keybindForOs(os: OperatingSystem): (modifers: ModifierKeys[], keys: string[]) => string {
	return (modifers: ModifierKeys[], keys: string[]) => keybind(modifers, keys, os);
}
