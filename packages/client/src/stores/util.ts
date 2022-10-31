import { proxy, subscribe } from 'valtio';
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

// The `valtio-persist` library is not working so this is a small alternative for us to use.
export function valtioPersist<T extends object>(localStorageKey: string, initialObject?: T): T {
	const d = localStorage.getItem(localStorageKey);
	const p = proxy(d !== null ? JSON.parse(d) : initialObject);
	subscribe(p, () => localStorage.setItem(localStorageKey, JSON.stringify(p)));
	return p;
}
