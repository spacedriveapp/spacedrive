import { useSnapshot } from 'valtio';

import { valtioPersist } from './util';

const appThemeStore = valtioPersist('appTheme', {
	themeName: 'vanilla',
	themeMode: 'light' as 'light' | 'dark',
	syncThemeWithSystem: false,
	hueValue: null as number | null
});

export function useThemeStore() {
	return useSnapshot(appThemeStore);
}

export function getThemeStore() {
	return appThemeStore;
}
