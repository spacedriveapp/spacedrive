import { useSnapshot } from 'valtio';
import { valtioPersist } from './util';

export type Themes = 'vanilla' | 'dark';

const themeStore = valtioPersist('sd-theme', {
	theme: 'vanilla' as Themes,
	syncThemeWithSystem: false,
	hueValue: null as number | null
});

export function useThemeStore() {
	return useSnapshot(themeStore);
}

export function getThemeStore() {
	return themeStore;
}
