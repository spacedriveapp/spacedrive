import { proxy, useSnapshot } from 'valtio';
import proxyWithPersist, { PersistStrategy, ProxyPersistStorageEngine } from 'valtio-persist';

const storage: ProxyPersistStorageEngine = {
	getItem: (name) => window.localStorage.getItem(name),
	setItem: (name, value) => window.localStorage.setItem(name, value),
	removeItem: (name) => window.localStorage.removeItem(name),
	getAllKeys: () => Object.keys(window.localStorage)
};

const appThemeStore = proxyWithPersist({
	// must be unique, files/paths will be created with this prefix
	name: 'appTheme',
	version: 0,
	initialState: {
		themeName: 'vanilla',
		themeMode: 'light' as 'light' | 'dark',
		syncThemeWithSystem: false,
		hueValue: null as number | null
	},
	persistStrategies: PersistStrategy.SingleFile,
	migrations: {},
	getStorage: () => storage
});

export function useThemeStore() {
	return useSnapshot(appThemeStore);
}

export function getThemeStore() {
	return appThemeStore;
}
