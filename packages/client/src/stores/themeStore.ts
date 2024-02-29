import { deepEqual } from 'fast-equals';
import { useRef } from 'react';
import { createMutable } from 'solid-js/store';

import { createPersistedMutable, useObserver, useSolidStore } from '../solid';

export type Themes = 'vanilla' | 'dark';

export const themeStore = createPersistedMutable(
	'sd-theme',
	createMutable({
		theme: 'dark' as Themes,
		syncThemeWithSystem: false,
		hueValue: 235
	})
);

export function useThemeStore() {
	return useSolidStore(themeStore);
}

export function useSubscribeToThemeStore(callback: () => void) {
	const ref = useRef<typeof themeStore>(themeStore);
	useObserver(() => {
		// Subscribe to store
		const store = { ...themeStore };

		// Only trigger React if it did in fact change.
		if (!deepEqual(store, ref.current)) {
			ref.current = store;
			callback();
		}
	});
}

export function isDarkTheme() {
	return themeStore.theme === 'dark';
}
