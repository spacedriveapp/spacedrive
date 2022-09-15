import { ProxyPersistStorageEngine } from 'valtio-persist';

export function resetStore<T extends Record<string, any>, E extends Record<string, any>>(
	store: T,
	defaults: E
) {
	for (const key in defaults) {
		// @ts-ignore
		store[key] = defaults[key];
	}
}

export const storageEngine: ProxyPersistStorageEngine = {
	getItem: (name) => window.localStorage.getItem(name),
	setItem: (name, value) => window.localStorage.setItem(name, value),
	removeItem: (name) => window.localStorage.removeItem(name),
	getAllKeys: () => Object.keys(window.localStorage)
};
