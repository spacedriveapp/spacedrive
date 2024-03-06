import { DependencyList } from 'react';
import { HotkeyCallback, Options, useHotkeys } from 'react-hotkeys-hook';

interface UseKeyBindOptions extends Options {
	repeatable?: boolean;
}

type UseKeyBindOptionsOrDependencyArray = UseKeyBindOptions | DependencyList;

export const useKeybind = (
	keys: string | string[] | string[][],
	callback: HotkeyCallback,
	options?: UseKeyBindOptionsOrDependencyArray,
	dependencies?: UseKeyBindOptionsOrDependencyArray
) => {
	const keyCombination = Array.isArray(keys)
		? Array.isArray(keys[0])
			? keys.map((k) => (k as string[]).join('+'))
			: keys.join('+')
		: keys;

	const repeatable =
		typeof options === 'object' && 'repeatable' in options
			? options.repeatable
			: typeof dependencies === 'object' && 'repeatable' in dependencies
				? dependencies.repeatable
				: false;

	return useHotkeys(
		keyCombination,
		(e, k) => (repeatable || !e.repeat) && callback(e, k),
		options,
		dependencies
	);
};
