import { useSnapshot } from 'valtio';

import { valtioPersist } from '../lib';

export type Themes = 'vanilla' | 'dark';

export type CoordinatesFormat = 'dd' | 'dms';

const themeStore = valtioPersist('sd-theme', {
	theme: 'dark' as Themes,
	syncThemeWithSystem: false,
	hueValue: 235,
	coordinatesFormat: 'dd' as CoordinatesFormat
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
