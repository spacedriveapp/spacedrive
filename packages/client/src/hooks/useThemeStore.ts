import { useObserver } from 'react-solid-state';
import { createMutable } from 'solid-js/store';

import { createPersistedMutable, useSolidStore } from '../solidjs-interop';

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
	useObserver(() => {
		// Subscribe to store
		const _ = { ...themeStore };

		callback();
	});
}

export function isDarkTheme() {
	return themeStore.theme === 'dark';
}
