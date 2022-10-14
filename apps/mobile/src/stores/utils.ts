import AsyncStorage from '@react-native-async-storage/async-storage';
import type { ProxyPersistStorageEngine } from 'valtio-persist';

export const StorageEngine: ProxyPersistStorageEngine = {
	getItem: (name) => AsyncStorage.getItem(name),
	setItem: (name, value) => AsyncStorage.setItem(name, value),
	removeItem: (name) => AsyncStorage.removeItem(name),
	getAllKeys: () => AsyncStorage.getAllKeys() as Promise<string[]>
};

export function resetStore<T extends Record<string, any>, E extends Record<string, any>>(
	store: T,
	defaults: E
) {
	for (const key in defaults) {
		// @ts-ignore
		store[key] = defaults[key];
	}
}
