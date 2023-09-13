import { useSnapshot } from 'valtio';

import { valtioPersist } from '../lib';

export type Themes = 'vanilla' | 'dark';

const themeStore = valtioPersist('sd-theme', {
	theme: 'dark' as Themes,
	syncThemeWithSystem: false,
	hueValue: 235
});

export function useThemeStore() {
	return useSnapshot(themeStore);
}

export function getThemeStore() {
	return themeStore;
}

export function isDarkTheme() {
	return themeStore.theme === 'dark';
}
